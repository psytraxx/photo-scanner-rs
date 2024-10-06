use anyhow::{anyhow, Result};
use base64::{prelude::BASE64_STANDARD, Engine};
use bytes::Bytes;
use exif::experimental::Writer;
use exif::{Field, In, Tag, Value};
use futures::stream::{FuturesUnordered, Stream, StreamExt};
use img_parts::jpeg::Jpeg;
use img_parts::ImageEXIF;
use photo_scanner_rust::domain::ports::Chat;
use photo_scanner_rust::outbound::openai::OpenAI;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use tokio::fs::{self, File};
use tokio::io::AsyncReadExt;
use tokio::sync::Semaphore;
use tracing::error;

// Function to list files in a directory and its subdirectories
fn list_files(directory: PathBuf) -> Pin<Box<dyn Stream<Item = Result<PathBuf>> + Send>> {
    let initial_read_dir = fs::read_dir(directory);

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
async fn process_image(path: &PathBuf) -> Result<Option<String>> {
    // Open the file
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

    // Try to parse EXIF data
    let jpeg = match Jpeg::from_bytes(buffer.clone().into()) {
        Ok(jpeg) => jpeg,
        Err(e) => {
            error!("Failed to read JPEG from {}: {}", path.display(), e);
            return Ok(None);
        }
    };

    let exif_bytes = match jpeg.exif() {
        Some(exif) => exif,
        None => {
            error!("No Exif data found in {}", path.display());
            return Ok(None);
        }
    };

    let mut exif = match exif::parse_exif(&exif_bytes) {
        Ok((data, success)) => {
            if success {
                data
            } else {
                error!("Failed to read Exif from {}", path.display());
                return Ok(None);
            }
        }
        Err(e) => {
            error!("Failed to read JPEG from {}: {}", path.display(), e);
            return Ok(None);
        }
    };

    let chat: OpenAI = OpenAI::new();

    let image_base64 = BASE64_STANDARD.encode(buffer);
    let folder_name: Option<String> = path
        .parent()
        .and_then(|p| p.file_name()?.to_str().map(|s| s.to_string()));

    let description = chat.get_chat(image_base64, None, folder_name).await?;
    // Add a new EXIF field (for example, a custom UserComment tag)
    let field_description = Field {
        tag: Tag::ImageDescription,
        ifd_num: In::PRIMARY,
        value: Value::Ascii(vec![description.as_bytes().to_vec()]),
    };

    // Add or replace the field in the EXIF data
    exif.push(field_description.clone());

    let mut writer = Writer::new();
    let exif_ref: &Vec<Field> = exif.as_ref();
    for f in exif_ref.iter() {
        writer.push_field(f);
    }

    let mut new_exif_bytes = std::io::Cursor::new(Vec::new());
    writer.write(&mut new_exif_bytes, false)?;
    let new_exif_bytes = new_exif_bytes.into_inner();

    let mut new_jpeg = jpeg.clone();
    new_jpeg.set_exif(Some(Bytes::from(new_exif_bytes)));

    let output_file = std::fs::File::create("/home/eric/updated_image2.jpg")?;
    new_jpeg.encoder().write_to(output_file)?;

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
                    match process_image(&file).await {
                        Ok(description) => match description {
                            Some(desciption) => {
                                tracing::info!("{} {}", &file.display(), desciption)
                            }
                            None => tracing::warn!("{} No EXIF data found", &file.display()),
                        },
                        Err(e) => {
                            error!("Error extracting EXIF from {}: {}", file.display(), e)
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
