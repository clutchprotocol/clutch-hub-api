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

use super::clutch_node_client::ClutchNodeClient;
use super::configuration::AppConfig;

pub fn build_schema(
    ws_manager: Arc<ClutchNodeClient>,
    config: AppConfig,
) -> Schema<Query, Mutation, Subscription> {
    Schema::build(
        Query::default(),
        Mutation::default(),
        Subscription::default(),
    )
    .data(ws_manager)
    .data(config)
    .finish()
}
