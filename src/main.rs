use anyhow::{anyhow, Result};
use futures::stream::{FuturesUnordered, Stream, StreamExt};
use photo_scanner_rust::domain::ports::Chat;
use photo_scanner_rust::outbound::exif::write_exif_description;
use photo_scanner_rust::outbound::image_provider::resize_and_base64encode_image;
use photo_scanner_rust::outbound::openai::OpenAI;
use photo_scanner_rust::outbound::xmp::{extract_persons, write_xmp_description};
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

// Function to check if the path has a valid JPEG extension
fn is_jpeg(path: &Path) -> bool {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => matches!(ext.to_ascii_lowercase().as_str(), "jpg" | "jpeg"),
        None => false, // No extension present
    }
}

// Function to extract EXIF data from a file
async fn extract_image_description(path: &Path, persons: &[String]) -> Result<String> {
    let chat: OpenAI = OpenAI::new();
    let image_base64 = resize_and_base64encode_image(path).unwrap();

    let folder_name: Option<String> = path
        .parent()
        .and_then(|p| p.file_name()?.to_str().map(|s| s.to_string()));
    chat.get_chat(&image_base64, persons, &folder_name).await
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

    let root_path = PathBuf::from(&args[1]);

    // Traverse the files and print them
    let mut files_stream = list_files(root_path);

    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_TASKS));
    let mut tasks = FuturesUnordered::new();

    while let Some(file_result) = files_stream.next().await {
        match file_result {
            Ok(path) => {
                if !is_jpeg(&path) {
                    // Skip files that are not JPEG
                    continue;
                }

                let semaphore = Arc::clone(&semaphore);

                tasks.push(tokio::spawn(async move {
                    let permit = semaphore.acquire().await.unwrap();

                    let start_time = Instant::now();

                    // Extract persons from the file
                    let persons = match extract_persons(&path) {
                        Ok(persons) => persons,
                        Err(e) => {
                            error!("Error extracting persons from {}: {}", &path.display(), e);
                            Vec::new()
                        }
                    };

                    match extract_image_description(&path, &persons).await {
                        Ok(description) => {
                            let duration = Instant::now() - start_time;
                            info!(
                                "Description for {}: {} Time taken: {:.2} seconds",
                                &path.display(),
                                &description,
                                duration.as_secs_f64()
                            );
                            match write_xmp_description(&description, &path) {
                                Ok(_) => {
                                    debug!("Wrote XMP {} {}", &path.display(), &description)
                                }
                                Err(e) => {
                                    error!(
                                        "Error storing XMP description for {}: {}",
                                        &path.display(),
                                        e
                                    )
                                }
                            }

                            match write_exif_description(&description, &path) {
                                Ok(_) => {
                                    debug!("Wrote EXIF {} {}", &path.display(), &description)
                                }
                                Err(e) => {
                                    error!(
                                        "Error storing EXIF description for {}: {}",
                                        &path.display(),
                                        e
                                    )
                                }
                            }
                        }
                        Err(e) => {
                            error!(
                                "Error extracting image description from {}: {}",
                                &path.display(),
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
