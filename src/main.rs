use anyhow::{anyhow, Result};
use futures::stream::{iter, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use photo_scanner_rust::domain::ports::Chat;
use photo_scanner_rust::outbound::image_provider::resize_and_base64encode_image;
use photo_scanner_rust::outbound::openai::OpenAI;
use photo_scanner_rust::outbound::xmp::{
    extract_persons, get_xmp_description, write_xmp_description,
};
use std::fs::read_dir;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tracing::{error, info, warn};

// Maximum number of concurrent tasks for ollama multimodal API
const MAX_CONCURRENT_TASKS: usize = 2;

/// Function to list files in a directory and its subdirectories.
fn list_files<P: AsRef<Path>>(path: P) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in read_dir(path)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            // Recursively traverse subdirectories
            files.extend(list_files(path)?);
        } else {
            files.push(path);
        }
    }
    Ok(files)
}

/// Function to check if the path has a valid JPEG extension.
fn is_jpeg(path: &Path) -> bool {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => matches!(ext.to_ascii_lowercase().as_str(), "jpg" | "jpeg"),
        None => false, // No extension present
    }
}

/// Main entry point.
#[tokio::main]
async fn main() -> Result<()> {
    // Set up tracing for logging.
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_ansi(true)
        .with_target(false)
        .without_time()
        .init();

    // Initialize the OpenAI chat model.
    let chat = Arc::new(OpenAI::new());

    // Get the folder path from command line arguments.
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        return Err(anyhow!("Please provide a path to the folder."));
    }
    let root_path = PathBuf::from(&args[1]);

    // Traverse the files and process them with limited concurrency.
    let files_list = list_files(root_path)?;

    // Create a progress bar with the total length of the vector.
    let progress_bar = Arc::new(ProgressBar::new(files_list.len() as u64));
    progress_bar.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")?,
    );

    iter(files_list)
        .for_each_concurrent(MAX_CONCURRENT_TASKS, |path| {
            let progress_bar = Arc::clone(&progress_bar);
            let chat = Arc::clone(&chat);
            async move {
                progress_bar.inc(1);
                // Skip files that do not need processing.
                if can_be_skipped(&path) {
                    return;
                }

                let start_time = Instant::now();

                // Extract persons from the image, handling any errors.
                let persons = match extract_persons(&path) {
                    Ok(persons) => persons,
                    Err(e) => {
                        warn!("Error extracting persons from {}: {}", path.display(), e);
                        Vec::new() // Default to an empty list if extraction fails.
                    }
                };

                // Resize and encode the image as base64.
                let image_base64 = match resize_and_base64encode_image(&path) {
                    Ok(encoded) => encoded,
                    Err(e) => {
                        error!("Error encoding image {}: {}", path.display(), e);
                        return;
                    }
                };

                // Optionally get the folder name for additional context.
                let folder_name: Option<String> = path
                    .parent()
                    .and_then(|p| p.file_name()?.to_str().map(str::to_string));

                // Generate a description using the chat model.
                let description = match chat.get_chat(&image_base64, &persons, &folder_name).await {
                    Ok(desc) => desc,
                    Err(e) => {
                        error!("Error generating description for {}: {}", path.display(), e);
                        return;
                    }
                };

                /* if let Err(e) = chat.get_embedding(&description).await {
                    error!("Error getting embedding for {}: {}", &path.display(), e);
                } */

                if let Err(e) = write_xmp_description(&description, &path) {
                    error!(
                        "Error storing XMP description for {}: {}",
                        path.display(),
                        e
                    );
                }

                // Log the time taken and other details.
                let duration = Instant::now() - start_time;
                info!(
                    "Generated \"{}\" for \"{}\", Time taken: {:.2} seconds, Persons: {:?}",
                    description,
                    path.display(),
                    duration.as_secs_f64(),
                    persons
                );
            }
        })
        .await;

    progress_bar.finish_with_message("All items have been processed.");

    Ok(())
}

/// Function to check if the file can be skipped.
fn can_be_skipped(path: &Path) -> bool {
    if !is_jpeg(path) {
        return true; // Skip non-JPEG files.
    }

    // Skip files that already have an XMP description.
    match get_xmp_description(path) {
        Ok(Some(description)) => {
            info!(
                "Description \"{}\" exists for \"{}\"",
                description,
                path.display()
            );
            true
        }
        Ok(None) => false, // No description present, proceed with processing.
        Err(e) => {
            error!(
                "Error getting XMP description for {}: {}",
                path.display(),
                e
            );
            true // Skip processing if there's an error retrieving the description.
        }
    }
}
