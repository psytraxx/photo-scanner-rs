use anyhow::{anyhow, Result};
use photo_scanner::domain::models::VectorOutputListUtils;
use photo_scanner::domain::ports::{Chat, VectorDB};
use photo_scanner::outbound::openai::OpenAI;
use photo_scanner::outbound::qdrant::QdrantClient;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

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

    let vector_db = Arc::new(QdrantClient::new()?);

    // Get the folder path from command line arguments.
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        return Err(anyhow!("Please provide question"));
    }
    let question = &args[1];
    let embeddings = chat.get_embeddings(vec![question.to_string()]).await?;

    let mut result = vector_db
        .search_points("photos", embeddings[0].as_slice(), HashMap::new())
        .await?;

    // Sort the results by score.
    result.sort_by_score();

    if result.is_empty() {
        warn!(
            "{:?}",
            "Please check your search input - no matching documents found"
        );
        return Ok(());
    }

    let result: Vec<String> = result
        .iter()
        .map(|r| r.payload.get("description").cloned().unwrap_or_default())
        .collect();

    debug!("{:?}", result);

    let result = chat.process_search_result(question, &result).await?;

    info!("{}", result);

    Ok(())
}
