use anyhow::{anyhow, Result};
use base64::{prelude::BASE64_STANDARD, Engine};
use bytes::Bytes;
use exif::experimental::Writer;
use exif::{Field, In, Tag, Value};
use futures::stream::{Stream, StreamExt};
use img_parts::jpeg::Jpeg;
use img_parts::ImageEXIF;
use photo_scanner_rust::domain::ports::Chat;
use photo_scanner_rust::outbound::openai::OpenAI;
use std::path::PathBuf;
use std::pin::Pin;
use tokio::fs::{self, File};
use tokio::io::AsyncReadExt;

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
async fn extract_exif(path: &PathBuf) -> Result<Option<String>> {
    // Open the file
    let mut file = match File::open(path).await {
        Ok(file) => file,
        Err(e) => {
            tracing::error!("Failed to open file {}: {}", path.display(), e);
            return Ok(None);
        }
    };

    // Create a buffer to store the file content
    let mut buffer = Vec::new();

    // Read the entire file content into the buffer
    file.read_to_end(&mut buffer).await?;

    // Try to parse EXIF data
    let jpeg = match Jpeg::from_bytes(buffer.clone().into()) {
        Ok(jpeg) => jpeg,
        Err(e) => {
            tracing::error!("Failed to read JPEG from {}: {}", path.display(), e);
            return Ok(None);
        }
    };

    let exif_bytes = jpeg.exif().unwrap();

    let mut exif = match exif::parse_exif(&exif_bytes) {
        Ok((data, success)) => {
            if success {
                data
            } else {
                tracing::error!("Failed to read Exif from {}", path.display());
                return Ok(None);
            }
        }
        Err(e) => {
            tracing::error!("Failed to read JPEG from {}: {}", path.display(), e);
            return Ok(None);
        }
    };

    let chat: OpenAI = OpenAI::new();

    let image_base64 = BASE64_STANDARD.encode(buffer);
    let folder_name: String = path
        .parent()
        .unwrap()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    let description = chat.get_chat(image_base64, None, Some(folder_name)).await?;
    println!("Description: {}", description);
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

    let mut new_jpeg = jpeg.clone();
    let new_exif_bytes = new_exif_bytes.into_inner();

    new_jpeg.set_exif(Some(Bytes::from(new_exif_bytes)));
    // Step 8: Write the modified JPEG back to a new file
    let output_file = std::fs::File::create("/home/eric/updated_image2.jpg")?;

    match new_jpeg.encoder().write_to(output_file) {
        Ok(jpeg) => (),
        Err(e) => {
            tracing::error!("Failed to read JPEG from {}: {}", path.display(), e);
            return Ok(None);
        }
    }

    Ok(None)
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() != 2 {
        return Err(anyhow!("Please provide a path to the folder."));
    }

    let path = PathBuf::from(&args[1]);

    // Traverse the files and print them
    let mut files_stream = list_files(path);
    while let Some(file_result) = files_stream.next().await {
        match file_result {
            Ok(file) => {
                let exif = extract_exif(&file).await?;
                println!("{} {:?}", &file.display(), exif)
            }
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    Ok(())
}
