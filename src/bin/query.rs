use anyhow::Result;
use photo_scanner_rust::domain::ports::{Chat, VectorDB};
use photo_scanner_rust::outbound::openai::OpenAI;
use photo_scanner_rust::outbound::qdrant::QdrantClient;
use std::collections::HashMap;
use std::sync::Arc;
use tracing_appender::rolling;
use tracing_subscriber::EnvFilter;

const QDRANT_GRPC: &str = "http://dot.dynamicflash.de:6334";

/// Main entry point.
#[tokio::main]
async fn main() -> Result<()> {
    // Set up tracing for logging.
    let file_appender = rolling::never("logs", "query.log");
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(file_appender)
        .with_target(false)
        .without_time()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Initialize the OpenAI chat model.
    let chat = Arc::new(OpenAI::new());

    let vector_db = Arc::new(QdrantClient::new(QDRANT_GRPC, 1024)?);

    let question_embeddigs = chat.get_embedding("Pictures sunsets in asia").await?;

    vector_db
        .search_points("photos", HashMap::new(), question_embeddigs)
        .await?;

    Ok(())
}
