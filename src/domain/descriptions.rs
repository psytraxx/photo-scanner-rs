use super::{
    file_utils::list_jpeg_files,
    ports::{Chat, ImageEncoder, XMPMetadata},
};
use anyhow::Result;
use futures::{stream::iter, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};
use tracing::{error, info, warn};

// Maximum number of concurrent tasks for multimodal API
const MAX_CONCURRENT_TASKS: usize = 2;

pub struct DescriptionService {
    image_provider: Arc<dyn ImageEncoder>,
    chat: Arc<dyn Chat>,
    xmp_metadata: Arc<dyn XMPMetadata>,
}

impl DescriptionService {
    pub fn new(
        image_provider: Arc<dyn ImageEncoder>,
        chat: Arc<dyn Chat>,
        xmp_metadata: Arc<dyn XMPMetadata>,
    ) -> Self {
        DescriptionService {
            image_provider,
            chat,
            xmp_metadata,
        }
    }

    pub async fn generate(&self, root_path: &PathBuf) -> Result<u64> {
        // Traverse the files and process them with limited concurrency.
        let files_list = list_jpeg_files(root_path)?;

        // Create a progress bar with the total length of the vector.
        let progress_bar = Arc::new(ProgressBar::new(files_list.len() as u64));
        progress_bar.set_style(
            ProgressStyle::default_bar().template(
                "Processing {msg} [{elapsed_precise}] [{wide_bar}] {pos}/{len} ({eta})",
            )?,
        );

        iter(files_list)
            .for_each_concurrent(MAX_CONCURRENT_TASKS, |path| {
                let progress_bar = Arc::clone(&progress_bar);
                let message = path
                    .parent()
                    .expect("Failed to get parent directory ")
                    .display()
                    .to_string();
                async move {
                    progress_bar.inc(1);
                    progress_bar.set_message(message);

                    // Skip files that do not need processing.
                    let description = self.xmp_metadata.get_description(&path).unwrap_or_default();
                    if can_be_skipped(description, &path) {
                        return;
                    }

                    let start_time = Instant::now();

                    // Extract persons from the image, handling any errors.
                    let persons = match self.xmp_metadata.get_persons(&path) {
                        Ok(persons) => persons,
                        Err(e) => {
                            warn!("Error extracting persons from {}: {}", path.display(), e);
                            Vec::new() // Default to an empty list if extraction fails.
                        }
                    };

                    // Resize and encode the image as base64.
                    let image_base64 =
                        match self.image_provider.resize_and_base64encode_image(&path) {
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
                    let description = match self
                        .chat
                        .get_image_description(&image_base64, &persons, &folder_name)
                        .await
                    {
                        Ok(desc) => desc,
                        Err(e) => {
                            error!("Error generating description for {}: {}", path.display(), e);
                            return;
                        }
                    };

                    /* if let Err(e) = chat.get_embedding(&description).await {
                        error!("Error getting embedding for {}: {}", &path.display(), e);
                    } */

                    if let Err(e) = self.xmp_metadata.set_description(&path, &description) {
                        error!(
                            "Error storing XMP description for {}: {}",
                            path.display(),
                            e
                        );
                    }

                    // Log the time taken and other details.
                    let duration = Instant::now() - start_time;
                    info!(
                        "Generated: [{}] \"{}\", Time taken: {:.2} seconds, Persons: {:?}",
                        path.display(),
                        description,
                        duration.as_secs_f64(),
                        persons
                    );
                }
            })
            .await;

        progress_bar.finish_with_message("All items have been processed.");

        Ok(progress_bar.position())
    }
}

/// Function to check if the file can be skipped.
fn can_be_skipped(description: Option<String>, path: &Path) -> bool {
    // Skip files that already have an XMP description.
    match description {
        Some(description) => {
            let re = Regex::new(r"(?i)\b(image|photo|picture|photograph)\b").unwrap();
            if re.is_match(&description) {
                info!("Reprocessed: [{}] \"{}\"", path.display(), description,);
                false
            } else {
                info!("Exists: [{}] \"{}\"", path.display(), description,);
                true
            }
        }
        None => false, //no description - no skip!
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        domain::{
            descriptions::{can_be_skipped, DescriptionService},
            ports::XMPMetadata,
        },
        outbound::{
            image_provider::ImageCrateEncoder, test_mocks::tests::ChatMock, xmp::XMPToolkitMetadata,
        },
    };
    use anyhow::Result;
    use std::{
        fs::{copy, remove_file},
        path::{Path, PathBuf},
        sync::Arc,
    };
    #[tokio::test]
    async fn test_generate_descriptions() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;

        let destination_file_path1 = temp_dir.path().join("example-full.jpg");
        // Copy an existing JPEG file to the temporary directory
        let source_file = PathBuf::from("testdata/example-full.jpg");
        copy(&source_file, &destination_file_path1)?;

        let destination_file_path2 = temp_dir.path().join("example-persons.jpg");
        // Copy an existing JPEG file to the temporary directory
        let source_file = PathBuf::from("testdata/example-persons.jpg");
        copy(&source_file, &destination_file_path2)?;

        let destination_file_path3 = temp_dir.path().join("example-existing-description-xmp.jpg");
        // Copy an existing JPEG file to the temporary directory
        let source_file = PathBuf::from("testdata/example-existing-description-xmp.jpg");
        copy(&source_file, &destination_file_path3)?;

        // Initialize dependencies
        let image_provider = Arc::new(ImageCrateEncoder::new());
        let chat = Arc::new(ChatMock);
        let xmp_metadata = Arc::new(XMPToolkitMetadata::new());

        // Create the DescriptionService instance
        let service = DescriptionService::new(image_provider, chat, xmp_metadata.clone());

        // Generate descriptions for the files in the temporary directory
        let result = service.generate(&temp_dir.path().into()).await;
        assert!(result.is_ok());
        // we should have processed 3 files
        assert_eq!(result.unwrap(), 3);

        let contents = xmp_metadata.get_description(&destination_file_path1)?;

        // Verify the content of the XMP file
        assert_eq!(contents, Some("description".to_string()));

        // Clean up by deleting the temporary file(s)
        remove_file(&destination_file_path1)?;
        remove_file(&destination_file_path2)?;

        Ok(())
    }

    #[test]
    fn test_can_be_skipped() {
        // Test case 1: No description
        assert!(!can_be_skipped(None, Path::new("test.jpg")));

        // Test case 2: Description without image-related keywords
        assert!(can_be_skipped(
            Some("random description".to_string()),
            Path::new("test.jpg")
        ));

        // Test case 3: Description with "image" keyword
        assert!(!can_be_skipped(
            Some("this is an image of nature".to_string()),
            Path::new("test.jpg")
        ));

        // Test case 4: Description with "photo" keyword
        assert!(!can_be_skipped(
            Some("beautiful photo".to_string()),
            Path::new("test.jpg")
        ));

        // Test case 5: Description with "PICTURE" keyword (case insensitive)
        assert!(!can_be_skipped(
            Some("This PICTURE shows mountains".to_string()),
            Path::new("test.jpg")
        ));

        // Test case 6: Description with "photograph" keyword
        assert!(!can_be_skipped(
            Some("A photograph of sunset".to_string()),
            Path::new("test.jpg")
        ));
    }
}
