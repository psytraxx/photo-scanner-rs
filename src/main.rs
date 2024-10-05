use anyhow::{anyhow, Result};
use exif::{Reader, Tag};
use futures::stream::{Stream, StreamExt};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::pin::Pin;
use tokio::fs;
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
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) => {
            tracing::error!("Failed to open file {}: {}", path.display(), e);
            return Ok(None);
        }
    };

    // Read the contents of the file into memory
    let mut reader = BufReader::new(&file);

    // Try to parse EXIF data
    let exif = match Reader::new().read_from_container(&mut reader) {
        Ok(exif) => exif,
        Err(e) => {
            tracing::error!("Failed to read EXIF data from {}: {}", path.display(), e);
            return Ok(None);
        }
    };

    //exif.fields().for_each(|f| println!("{:?}", f));

    // For demonstration, we'll try to extract the camera make (you can extract other tags as needed)
    if let Some(field) = exif.get_field(Tag::DateTimeOriginal, exif::In::PRIMARY) {
        Ok(Some(format!(
            "{}: {}",
            Tag::DateTimeOriginal,
            field.display_value().with_unit(&exif)
        )))
    } else {
        Ok(None)
    }
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
