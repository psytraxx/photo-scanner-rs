use anyhow::{anyhow, Result};
use photo_scanner_rust::domain::descriptions::DescriptionService;
use photo_scanner_rust::outbound::image_provider::ImageCrateEncoder;
use photo_scanner_rust::outbound::openai::OpenAI;
use photo_scanner_rust::outbound::xmp::XMPToolkitMetadata;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_appender::rolling;
use tracing_subscriber::EnvFilter;

/// Main entry point.
#[tokio::main]
async fn main() -> Result<()> {
    // Set up tracing for logging.
    let file_appender = rolling::never("logs", "descriptions.log");
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(file_appender)
        .with_target(false)
        .without_time()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Initialize the OpenAI chat model.
    let chat = Arc::new(OpenAI::new());

    // Initialize the image provider
    let image_provider = Arc::new(ImageCrateEncoder::new());

    let xmp_toolkit = Arc::new(XMPToolkitMetadata::new());

    // Get the folder path from command line arguments.
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        return Err(anyhow!("Please provide a path to the folder."));
    }
    let root_path = PathBuf::from(&args[1]);

    let service = DescriptionService::new(image_provider, chat, xmp_toolkit);

    service.generate(&root_path).await
}
