# Clutch Hub API

![Alpha](https://img.shields.io/badge/status-alpha-orange.svg)
![Experimental](https://img.shields.io/badge/stage-experimental-red.svg)
![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)

> ⚠️ **ALPHA SOFTWARE** — APIs may change without notice.

Rust backend bridging applications to the Clutch Node blockchain via GraphQL.

**Documentation:** https://docs.clutchprotocol.io/clutch-hub-api/overview

## Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Liveness check |
| `/graphql` | POST | Queries and mutations |
| `/graphql/ws` | GET | GraphQL subscriptions (WebSocket) |

## Authentication

Wallet-based JWT — no username/password. Token issuance requires proof of key ownership: sign the challenge `clutch-auth:{publicKey}:{timestamp}` (unix seconds, ±120s window) with the wallet's private key using the stack's signing convention (the SDK does this automatically):

```graphql
mutation {
  generateToken(
    publicKey: "0x..."
    timestamp: 1751500000
    signature: { r: "0x...", s: "0x...", v: 28 }
  ) { token expiresAt }
}
```

Protected mutations require `Authorization: Bearer <token>`.

## GraphQL highlights

**Queries:** `listRideRequests`, `listRideOffers`, `listActiveTrips`, `listCompletedTrips`, `listRecentTrips`, `accountBalance`

**Mutations:** `createUnsignedRideRequest`, `createUnsignedRideOffer`, `createUnsignedRideAcceptance`, `createUnsignedRidePay`, `createUnsignedRideCancel`, `createUnsignedRideRequestCancel`, `sendRawTransaction`

**Subscriptions:** `rideRequestsUpdated`, `rideOffersUpdated`, `activeTripsUpdated`, `completedTripsUpdated`, `recentTripsUpdated`, `accountBalanceUpdated`

## Quick start

```bash
cp env.example .env
cargo run
# or
docker compose up --build
```

Default: http://localhost:3000/health

Full stack: [clutch-deploy](https://github.com/clutchprotocol/clutch-deploy)

## Configuration

TOML files in `config/` with `APP_` environment overrides. Key settings: `ws_addr`, `clutch_node_ws_url`, `jwt_secret`, `allowed_origins`.

See [API Configuration](https://docs.clutchprotocol.io/clutch-hub-api/configuration).

## Docker

```bash
docker pull ghcr.io/clutchprotocol/clutch-hub-api:latest
```

**Created and maintained by [Mehran Mazhar](https://github.com/MehranMazhar)**
