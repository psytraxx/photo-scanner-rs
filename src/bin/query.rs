use anyhow::Result;
use photo_scanner_rust::domain::ports::{Chat, VectorDB};
use photo_scanner_rust::outbound::openai::OpenAI;
use photo_scanner_rust::outbound::qdrant::QdrantClient;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;

const QDRANT_GRPC: &str = "http://dot.dynamicflash.de:6334";

/// Main entry point.
#[tokio::main]
async fn main() -> Result<()> {
    // Set up tracing for logging.
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .without_time()
        .with_writer(std::io::stdout)
        .init();

    // Initialize the OpenAI chat model.
    let chat = Arc::new(OpenAI::new());

    let vector_db = Arc::new(QdrantClient::new(QDRANT_GRPC, 1024)?);

    // what is our favorite beach holiday destination in europe
    // which festivals has annina visited in in the last years
    let question = "which cities did we visit in japan";

    let question_embeddigs = chat.get_embedding(question).await?;

    let result = vector_db
        .search_points("photos", &question_embeddigs, HashMap::new())
        .await?;

    let result: Vec<String> = result
        .iter()
        .map(|r| r.payload.get("description").cloned().unwrap_or_default())
        .collect();

    info!("{:?}", result);

    let result = chat.process_search_result(question, &result).await?;

    info!("{:?}", result);

    Ok(())
}
