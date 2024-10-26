use anyhow::{anyhow, Result};
use futures::future::join_all;
use futures::stream::{Stream, StreamExt};
use photo_scanner_rust::domain::ports::Chat;
use photo_scanner_rust::outbound::image_provider::resize_and_base64encode_image;
use photo_scanner_rust::outbound::openai::OpenAI;
use photo_scanner_rust::outbound::xmp::{
    extract_persons, get_xmp_description, write_xmp_description,
};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;
use tokio::spawn;
use tokio::sync::Semaphore;
use tracing::{error, info, warn};

// Maximum number of concurrent tasks for ollama multimodal API
const MAX_CONCURRENT_TASKS: usize = 2;

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
    let mut tasks = Vec::new();

    while let Some(file_result) = files_stream.next().await {
        let path = match file_result {
            Ok(path) => path,
            Err(e) => {
                error!("Error: {}", e);
                continue;
            }
        };

        // no need to create a task if it can be skipped
        if can_be_skipped(&path) {
            continue;
        }

        let semaphore = Arc::clone(&semaphore);
        let chat: OpenAI = OpenAI::new();

        let task = spawn(async move {
            let permit = match semaphore.acquire().await {
                Ok(permit) => permit,
                Err(e) => {
                    error!("Failed to acquire semaphore for {}: {}", path.display(), e);
                    return;
                }
            };

            let persons = match extract_persons(&path) {
                Ok(persons) => persons,
                Err(e) => {
                    warn!("Error extracting persons from {}: {}", &path.display(), &e);
                    Vec::new()
                }
            };

            let image_base64 = resize_and_base64encode_image(&path).unwrap();

            let folder_name: Option<String> = path
                .parent()
                .and_then(|p| p.file_name()?.to_str().map(|s| s.to_string()));

            let start_time = Instant::now();
            let description = match chat.get_chat(&image_base64, &persons, &folder_name).await {
                Ok(description) => description,
                Err(e) => {
                    error!(
                        "Error extracting image description from {}: {}",
                        &path.display(),
                        &e
                    );
                    drop(permit);
                    return;
                }
            };

            let duration = Instant::now() - start_time;
            info!(
                "Generated \"{}\" for \"{}\", Time taken: {:.2} seconds, Persons: {:?}",
                &description,
                &path.display(),
                duration.as_secs_f64(),
                &persons
            );

            if let Err(e) = chat.get_embedding(&description).await {
                error!("Error getting embedding for {}: {}", &path.display(), e);
            }

            if let Err(e) = write_xmp_description(&description, &path) {
                error!(
                    "Error storing XMP description for {}: {}",
                    path.display(),
                    e
                );
            }

            drop(permit);
        });

        tasks.push(task)
    }

    // Wait for all the tasks to complete before exiting the method.
    let results = join_all(tasks).await;

    for result in results {
        if let Err(e) = result {
            error!("Task failed: {:?}", e);
        }
    }

    Ok(())
}

// Function to check if the file needs to be processed
fn can_be_skipped(path: &Path) -> bool {
    // do create task for non-jpeg files
    if !is_jpeg(path) {
        return true;
    }

    // Check if the EXIF description already exists and skip the file if it does.
    match get_xmp_description(path) {
        Ok(Some(description)) => {
            info!(
                "Description \"{}\" exists for \"{}\"",
                &description,
                &path.display()
            );
            return true;
        }
        Ok(None) => {
            // Continue code execution if there is no description
        }
        Err(e) => {
            error!(
                "Error getting XMP description for {}: {}",
                &path.display(),
                &e
            );
            return true;
        }
    }
    false
}
