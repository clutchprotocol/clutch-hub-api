pub mod handler;
pub mod lists;
pub mod mutation;
pub mod query;
pub mod subscription;
pub mod types;

use std::sync::Arc;

pub use mutation::Mutation;
pub use query::Query;
pub use subscription::Subscription;

use async_graphql::Schema;

use super::clutch_node_client::{ChainInfo, ClutchNodeClient};
use super::configuration::AppConfig;

pub fn build_schema(
    ws_manager: Arc<ClutchNodeClient>,
    config: AppConfig,
    chain_info: Arc<ChainInfo>,
) -> Schema<Query, Mutation, Subscription> {
    Schema::build(
        Query::default(),
        Mutation::default(),
        Subscription::default(),
    )
    .data(ws_manager)
    .data(config)
    .data(chain_info)
    // Bound query cost so a single aliased request can't fan out into hundreds of
    // concurrent node RPCs contending on the one shared WebSocket mutex (a cheap DoS).
    // Every field is weight 1 by default; normal client queries are well under this.
    .limit_complexity(200)
    .limit_depth(12)
    .finish()
}
