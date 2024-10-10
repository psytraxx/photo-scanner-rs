use anyhow::{anyhow, Result};
use futures::stream::{FuturesUnordered, Stream, StreamExt};
use photo_scanner_rust::domain::ports::Chat;
use photo_scanner_rust::outbound::exif::write_exif_description;
use photo_scanner_rust::outbound::image_provider::resize_and_base64encode_image;
use photo_scanner_rust::outbound::openai::OpenAI;
use photo_scanner_rust::outbound::xmp::write_xmp_description;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Semaphore;
use tracing::{debug, error, info};

// Maximum number of concurrent tasks for ollama multimodal API
const MAX_CONCURRENT_TASKS: usize = 1;

// Function to list files in a directory and its subdirectories
fn list_files(directory: PathBuf) -> Pin<Box<dyn Stream<Item = Result<PathBuf>> + Send>> {
    let initial_read_dir = tokio::fs::read_dir(directory);

    // Create an initial stream that will be used for recursion
    let stream = async_stream::try_stream! {
        let mut read_dir = initial_read_dir.await?;
        while let Some(entry) = read_dir.next_entry().await? {
            let path = entry.path();
            if path.is_file() {
                // Yield the file path
                yield path;
            } else if path.is_dir() {
                // Recursively list files in the subdirectory
                let sub_stream = list_files(path);
                // Flatten the subdirectory stream into the current stream
                for await sub_path in sub_stream {
                    yield sub_path?;
                }
            }
        }
    };

    Box::pin(stream)
}

// Function to extract EXIF data from a file
async fn extract_image_description(path: &Path) -> Result<String> {
    let chat: OpenAI = OpenAI::new();

    // Convert extension to lowercase and check if it is "jpg" or "jpeg"
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
    {
        Some(ext) if ext == "jpg" || ext == "jpeg" => {}
        _ => {
            // If the file is not a JPEG/JPG, return Ok(None)
            return Err(anyhow!("Not a JPEG file"));
        }
    }

    let image_base64 = resize_and_base64encode_image(path).unwrap();

    let folder_name: Option<String> = path
        .parent()
        .and_then(|p| p.file_name()?.to_str().map(|s| s.to_string()));

    chat.get_chat(image_base64, None, folder_name).await
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_ansi(true)
        .with_target(false)
        .without_time()
        .init();

    let args: Vec<String> = std::env::args().collect();

    if args.len() != 2 {
        return Err(anyhow!("Please provide a path to the folder."));
    }

    let path = PathBuf::from(&args[1]);

    // Traverse the files and print them
    let mut files_stream = list_files(path);

    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_TASKS));
    let mut tasks = FuturesUnordered::new();

    while let Some(file_result) = files_stream.next().await {
        match file_result {
            Ok(file) => {
                let semaphore = Arc::clone(&semaphore);

                tasks.push(tokio::spawn(async move {
                    let permit = semaphore.acquire().await.unwrap();
                    
                    let start_time = Instant::now();
                    match extract_image_description(&file).await {
                        Ok(description) => {
                            let duration = Instant::now() - start_time;
                            info!("Description for {}: {} Time taken: {:.2} seconds", &file.display(), &description, duration.as_secs_f64());
                            match write_xmp_description(&description, &file) {
                                Ok(_) => {
                                    debug!("Wrote XMP {} {}", &file.display(), &description)
                                }
                                Err(e) => {
                                    error!(
                                        "Error storing XMP description for {}: {}",
                                        &file.display(),
                                        e
                                    )
                                }
                            }
                            match write_exif_description(&description, &file) {
                                Ok(_) => {
                                    debug!("Wrote EXIF {} {}", &file.display(), &description)
                                }
                                Err(e) => {
                                    error!(
                                        "Error storing EXIF description for {}: {}",
                                        &file.display(),
                                        e
                                    )
                                }
                            }
                        }
                        Err(e) => {
                            error!(
                                "Error extracting image description from {}: {}",
                                &file.display(),
                                e
                            )
                        }
                    }
                    drop(permit);
                }));
            }
            Err(e) => error!("Error: {}", e),
        }
    }

    // Await for all tasks to complete
    while let Some(result) = tasks.next().await {
        match result {
            Ok(_) => {
                // Task completed successfully, we could add additional logging here if needed
            }
            Err(e) => {
                tracing::error!("Task failed: {:?}", e);
            }
        }
    }

    Ok(())
}
