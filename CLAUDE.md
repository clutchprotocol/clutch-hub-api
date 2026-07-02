# clutch-hub-api

Rust GraphQL bridge between client apps and clutch-node. Actix-web serves HTTP/WS; async-graphql defines the schema; a persistent WebSocket JSON-RPC client talks to the node. See the parent `D:\source\clutch\CLAUDE.md` for the multi-repo picture — this file covers internals only.

## Commands

- Build: `cargo build` (release: `cargo build --release`; toolchain pinned to 1.86.0 in `rust-toolchain.toml`)
- Run: `cargo run` (loads `config/default.toml`) or `cargo run -- --env <name>` → `config/<name>.toml`
- Test: `cargo test` (unit tests only, inline in `src/hub/faucet.rs`, `src/hub/signature_keys.rs`, and `src/hub/auth.rs`; no integration tests). The `sdk_generated_fixture_*` tests in `auth.rs` pin cross-language agreement with SDK-signed auth challenges — regenerate them from the SDK signing code path if the challenge format ever changes.
- Docker: `docker compose up --build` (API only, port 3000); container smoke test: `.\test-api.ps1`

## Source layout (`src/`)

- `main.rs` — CLI arg parsing (`--env` flag or positional), loads config, starts tracing, metrics server, node WS client, then the Actix server.
- `hub/server.rs` — Actix `HttpServer`: routes `/health`, `/faucet` (POST), `/graphql` (POST), `/graphql/ws` (GET, subscriptions). CORS is allow-any-origin.
- `hub/graphql/mod.rs` — `build_schema()`: injects `Arc<ClutchNodeClient>` and `AppConfig` as schema data.
- `hub/graphql/query.rs` — `Query` root: `list_ride_requests`, `list_ride_offers`, `list_active_trips`, `list_completed_trips`, `list_recent_trips`, `account_balance`.
- `hub/graphql/mutation.rs` — `Mutation` root: `generate_token`, six `create_unsigned_*` ride builders, `send_raw_transaction`.
- `hub/graphql/subscription.rs` — `Subscription` root: `*_updated` streams. These are **polling loops**, not push: `stream::unfold` re-queries the node every 1000ms (`SNAPSHOT_INTERVAL_MS`; ride offers 500ms).
- `hub/graphql/lists.rs` — shared fetch+parse helpers (`list_*_parsed`) used by both queries and subscriptions; items that fail to deserialize are logged and silently dropped.
- `hub/graphql/types.rs` — GraphQL object/input types (incl. `AuthSignatureInput`), `AuthUser`, `AuthGuard`, `get_auth_user()`.
- `hub/graphql/handler.rs` — HTTP/WS handlers; JWT extraction and validation.
- `hub/clutch_node_client/` — node RPC client: `client.rs` (`send_request`, `get_next_nonce`, `get_account_balance`, `list_*`), `connection.rs` (reconnect loop, response demux), `mod.rs` (JSON-RPC request/response structs).
- `hub/auth.rs` — JWT generation (HS256, claims `{pk, exp}`) + `verify_auth_challenge` (proof-of-key-ownership check for `generateToken`).
- `hub/faucet.rs` — testnet faucet: builds, signs (RLP + Keccak-256 + secp256k1), and submits a Transfer from a funded key.
- `hub/signature_keys.rs` — secp256k1 keypair/sign/verify + `validate_public_key` (accepts 40-char address or 130-char uncompressed pubkey, optional 0x).
- `hub/referrer.rs` — `configured_referrer()`: trims config value, empty → `None`.
- `hub/configuration.rs` — `AppConfig` + loading. `hub/metric.rs` — Prometheus on a separate Axum server. `hub/tracing.rs` / `hub/seq.rs` — tracing to stdout + Seq.

## Request flow

1. Client POSTs `/graphql`; `graphql_handler` (handler.rs) validates the `Authorization: Bearer <JWT>` header and, if valid, inserts `AuthUser { public_key }` into the request data. Invalid/missing tokens do NOT fail the request — the operation only fails if it hits an `AuthGuard`.
2. Resolver pulls `Arc<ClutchNodeClient>` (and `AppConfig`) from `ctx.data`.
3. `client.send_request(method, params)` sends JSON-RPC over the shared node WebSocket with a UUID id, registers a oneshot in `pending_requests`, and awaits the reply with a **10s timeout**. `connection.rs` matches responses to ids; on disconnect it clears the sink, fails all pending requests, and reconnects every 5s.
4. For unsigned-tx mutations, the resolver fetches the next nonce from the node, then returns a JSON params blob (`{from, nonce, data:{function_call_type, arguments}}`) — the client signs it locally (SDK) and submits via `sendRawTransaction`.

### Adding a new GraphQL operation end-to-end

1. Add/extend a node RPC wrapper in `src/hub/clutch_node_client/client.rs` if a new node method is involved.
2. Define response/input types in `src/hub/graphql/types.rs` (`SimpleObject`/`InputObject`; field names auto-camelCase in the schema).
3. If shared by query + subscription, put fetch/parse logic in `src/hub/graphql/lists.rs`.
4. Add the resolver to `query.rs` / `mutation.rs` / `subscription.rs`; add `#[graphql(guard = "AuthGuard")]` if auth is required.
5. Downstream: update `clutch-hub-sdk-js`, then `clutch-hub-demo-app`, then `clutch-docs`.

## Auth

- `generateToken(publicKey, timestamp, signature)` mutation (no auth) → HS256 JWT with claims `{pk, exp}`, expiry `jwt_expiration_hours`. Requires **proof of key ownership**: the client signs the canonical challenge `clutch-auth:{publicKey}:{timestamp}` (`publicKey` byte-for-byte as sent, `timestamp` decimal unix seconds). The signature follows the stack's tx convention — Keccak-256 the message to a 64-char hex string, then recoverable secp256k1 over Keccak256 of that hex string's UTF-8 bytes (`auth.rs::auth_challenge_hash_hex` / SDK `signAuthChallenge`). Rejected when the timestamp is more than ±120s from server time (`AUTH_TIMESTAMP_WINDOW_SECS`) or the signature doesn't recover to `publicKey` (address or uncompressed-pubkey form, `SignatureKeys::verify_key_ownership`).
- Validation in `handler.rs::extract_auth_user_from_bearer` (also accepts a raw token without the `Bearer ` prefix). For subscriptions, the token comes from the WS `connection_init` payload (`Authorization` or `authorization` key), not HTTP headers.
- Guarded (require JWT): all `createUnsigned*` mutations, `sendRawTransaction`, `accountBalance`, `userRideRequests`. Everything else — including all subscriptions and the `list*` queries — is public.
- The `Claims` struct is duplicated in `auth.rs` and `handler.rs`; keep them in sync.

## Config

- `config/<env>.toml` selected by `--env` (default `default`); only `config/default.toml` is checked in. Env overrides use the `APP_` prefix (e.g. `APP_JWT_SECRET`, `APP_CLUTCH_NODE_WS_URL`); `.env` is loaded via dotenv.
- Key settings (`AppConfig` in `src/hub/configuration.rs`): `ws_addr` (main HTTP/GraphQL bind), `clutch_node_ws_url`, `jwt_secret`, `jwt_expiration_hours`, `serve_metric_addr`, `seq_url`/`seq_api_key`, `log_level`, `faucet_enabled`/`faucet_private_key`/`faucet_amount_clt`, `default_ride_request_referrer`/`default_ride_offer_referrer`.
- Ports: GraphQL/HTTP on **3000**, metrics on **9090** — the local `config/default.toml` and the Docker/deploy config (`clutch-deploy/config/api/default.toml`, mounted over `/app/config`) agree on these.
- `env.example` documents the working `APP_*` override names (e.g. `APP_JWT_SECRET`); copy it to `.env` and adjust.

## Faucet

`POST /faucet {"address": "0x..."}` (server.rs → faucet.rs). Requires `faucet_enabled = true` and a funded `faucet_private_key`. It checks the faucet balance, fetches the nonce, RLP-encodes and signs a Transfer exactly matching the SDK's `signTransaction` encoding (Keccak-256 over the *UTF-8 hex string* of the unsigned-tx hash — see `sign_transfer_raw_transaction`), and submits `send_raw_transaction`. The default faucet key's address must be funded in the node genesis (`0xdeb4cfb63db134698e1879ea24904df074726cc0`).

## Gotchas / conventions

- Referrers are injected server-side from config (`default_ride_*_referrer`); clients must never pass a referrer.
- `get_next_nonce` and `get_account_balance` swallow node errors and return fallbacks (nonce `1`, balance `0`) — a down node can produce transactions with wrong nonces rather than an error.
- `send_raw_transaction` params must be a bare JSON string, not an object; `client.rs::send_request` special-cases that method. Mutations auto-prepend `0x` to raw transactions.
- `userRideRequests` and `rideRequest` queries return hardcoded placeholder data.
- Node responses for list RPCs use snake_case field names deserialized into the GraphQL types via serde; keep the two in sync when node payloads change.
- Metrics (`hub/metric.rs`) run on a *separate* Axum server at `serve_metric_addr` (`/metrics`); the gauges defined there (`LATEST_BLOCK_INDEX`) are currently never updated.
- Logs go to Seq (`seq_url`) as well as stdout; request/response bodies to the node are logged at info level.
