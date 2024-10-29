use anyhow::{anyhow, Result};
use photo_scanner_rust::domain::embeddings::EmbeddingsService;
use photo_scanner_rust::outbound::openai::OpenAI;
use photo_scanner_rust::outbound::qdrant::QdrantClient;
use photo_scanner_rust::outbound::xmp::XMPToolkitMetadata;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_appender::rolling;
use tracing_subscriber::EnvFilter;

const QDRANT_GRPC: &str = "http://dot.dynamicflash.de:6334";

/// Main entry point.
#[tokio::main]
async fn main() -> Result<()> {
    // Set up tracing for logging.
    let file_appender = rolling::never("logs", "embeddings.log");
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(file_appender)
        .with_target(false)
        .without_time()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Initialize the OpenAI chat model.
    let chat = Arc::new(OpenAI::new());

    let xmp_toolkit = Arc::new(XMPToolkitMetadata::new());

    let vector_db = Arc::new(QdrantClient::new(QDRANT_GRPC, 1024)?);

    // Get the folder path from command line arguments.
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        return Err(anyhow!("Please provide a path to the folder."));
    }
    let root_path = PathBuf::from(&args[1]);

    let service = EmbeddingsService::new(chat, xmp_toolkit, vector_db);

    service.generate(&root_path).await
}
