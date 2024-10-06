use anyhow::{anyhow, Result};
use base64::{prelude::BASE64_STANDARD, Engine};
use futures::stream::{FuturesUnordered, Stream, StreamExt};
use photo_scanner_rust::domain::ports::Chat;
use photo_scanner_rust::outbound::openai::OpenAI;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::{fs::File, io::AsyncReadExt};
use tracing::{error, info};
use xmp_toolkit::{OpenFileOptions, XmpFile, XmpMeta, XmpValue};

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
                let sub_stream = list_files(path.clone());
                // Flatten the subdirectory stream into the current stream
                for await sub_path in sub_stream {
                    yield sub_path?;
                }
            }
        }
    };

    // Return the stream boxed to avoid type cycle issues
    Box::pin(stream)
}

// Function to extract EXIF data from a file
async fn extract_image_description(path: &PathBuf) -> Result<Option<String>> {
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
            return Ok(None);
        }
    }

    let mut file = match File::open(path).await {
        Ok(file) => file,
        Err(e) => {
            error!("Failed to open file {}: {}", path.display(), e);
            return Ok(None);
        }
    };

    // Create a buffer to store the file content
    let mut buffer = Vec::new();

    // Read the entire file content into the buffer
    match file.read_to_end(&mut buffer).await {
        Ok(_) => (),
        Err(e) => {
            error!("Failed to read file {}: {}", path.display(), e);
            return Ok(None);
        }
    };

    let image_base64 = BASE64_STANDARD.encode(buffer);

    let folder_name: Option<String> = path
        .parent()
        .and_then(|p| p.file_name()?.to_str().map(|s| s.to_string()));

    let description = chat.get_chat(image_base64, None, folder_name).await?;

    Ok(Some(description))
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

    let semaphore = Arc::new(Semaphore::new(2)); // Limit to 2 concurrent tasks
    let mut tasks = FuturesUnordered::new();

    while let Some(file_result) = files_stream.next().await {
        match file_result {
            Ok(file) => {
                let semaphore = Arc::clone(&semaphore);
                tasks.push(tokio::spawn(async move {
                    let permit = semaphore.acquire().await.unwrap();
                    match extract_image_description(&file).await {
                        Ok(description) => match description {
                            Some(description) => {
                                match store_description(&file, description.clone()).await {
                                    Ok(_) => {
                                        tracing::info!("{} {}", &file.display(), description)
                                    }
                                    Err(e) => {
                                        error!(
                                            "Error storing XMP description for {}: {}",
                                            &file.display(),
                                            e
                                        )
                                    }
                                }
                            }
                            None => tracing::warn!("Skipping processing of {}", &file.display()),
                        },
                        Err(e) => {
                            error!("Error extracting EXIF from {}: {}", &file.display(), e)
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

async fn store_description(path: &PathBuf, description: String) -> Result<()> {
    // Step 1: Open the JPEG file with XmpFile for reading and writing XMP metadata
    let mut xmp_file = XmpFile::new()?;

    xmp_file
        .open_file(
            path,
            OpenFileOptions::default()
                .only_xmp()
                .for_update()
                .use_smart_handler(),
        )
        .or_else(|_| {
            xmp_file.open_file(
                path,
                OpenFileOptions::default()
                    .only_xmp()
                    .for_update()
                    .use_packet_scanning(),
            )
        })?;

    // Step 2: Try to extract existing XMP metadata
    let mut xmp = if let Some(existing_xmp) = xmp_file.xmp() {
        info!("XMP metadata exists. Parsing it...");
        existing_xmp
    } else {
        info!("No XMP metadata found. Creating a new one.");
        XmpMeta::new()?
    };

    /*  xmp.iter(IterOptions::default()).for_each(|p| {
        info!("{:?}", p);
    });*/

    xmp.delete_property(xmp_toolkit::xmp_ns::DC, "description")?;

    let new_value: XmpValue<String> = XmpValue::new(description.clone());
    xmp.set_property(xmp_toolkit::xmp_ns::DC, "description", &new_value)?;

    xmp_file.put_xmp(&xmp)?;
    xmp_file.close();

    Ok(())
}
