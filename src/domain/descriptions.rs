use anyhow::Result;
use futures::{stream::iter, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};
use tracing::{error, info, warn};

use super::{
    file_utils::list_jpeg_files,
    ports::{Chat, ImageEncoder, XMPMetadata},
};

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

    pub async fn generate(&self, root_path: &PathBuf) -> Result<()> {
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
                let message = path.parent().unwrap().display().to_string();
                async move {
                    progress_bar.inc(1);
                    progress_bar.set_message(message);
                    // Skip files that do not need processing.
                    if self.can_be_skipped(&path) {
                        return;
                    }

                    let start_time = Instant::now();

                    // Extract persons from the image, handling any errors.
                    let persons = match self.xmp_metadata.extract_persons(&path) {
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

                    if let Err(e) = self.xmp_metadata.write_xmp_description(&description, &path) {
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
    fn can_be_skipped(&self, path: &Path) -> bool {
        // Skip files that already have an XMP description.
        match self.xmp_metadata.get_xmp_description(path) {
            Ok(Some(description)) => {
                if description.starts_with("The image")
                    || description.starts_with("The photo")
                    || description.starts_with("The scene")
                    || description.starts_with("This image")
                    || description.starts_with("In the image")
                    || description.starts_with("This scene")
                {
                    info!(
                        "Reprocessed \"{}\" exists for \"{}\", but will be ",
                        description,
                        path.display()
                    );
                    false
                } else {
                    info!(
                        "Description \"{}\" exists for \"{}\"",
                        description,
                        path.display()
                    );
                    true
                }
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
}

#[cfg(test)]
mod tests {

    use std::{
        fs::{copy, remove_file},
        path::PathBuf,
        sync::Arc,
    };

    use anyhow::Result;
    use async_trait::async_trait;

    use crate::{
        domain::{
            descriptions::DescriptionService,
            ports::{Chat, XMPMetadata},
        },
        outbound::{image_provider::ImageCrateEncoder, xmp::XMPToolkitMetadata},
    };
    #[tokio::test]
    async fn test_generate_descriptions() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let destination_file_path = temp_dir.path().join("4L2A3805.jpg");

        // Copy an existing JPEG file to the temporary directory
        let source_file = PathBuf::from("testdata/sizilien/4L2A3805.jpg");
        copy(&source_file, &destination_file_path)?;

        // Initialize dependencies
        let image_provider = Arc::new(ImageCrateEncoder::new());
        let chat = Arc::new(ChatMock);
        let xmp_metadata = Arc::new(XMPToolkitMetadata::new());

        // Create the DescriptionService instance
        let service = DescriptionService::new(image_provider, chat, xmp_metadata.clone());

        // Generate descriptions for the files in the temporary directory
        service.generate(&temp_dir.path().into()).await?;

        let contents = xmp_metadata.get_xmp_description(&destination_file_path)?;

        // Verify the content of the XMP file
        assert_eq!(contents, Some("description".to_string()));

        // Clean up by deleting the temporary file
        remove_file(&destination_file_path)?;

        Ok(())
    }

    struct ChatMock;

    #[async_trait]
    impl Chat for ChatMock {
        async fn get_image_description(
            &self,
            _image_base64: &str,
            _persons: &[String],
            _folder_name: &Option<String>,
        ) -> Result<String> {
            Ok("description".to_string())
        }

        // Mock implementation for get_embedding
        async fn get_embedding(&self, _text: &str) -> Result<Vec<f32>> {
            unimplemented!()
        }

        async fn process_search_result(
            &self,
            _question: &str,
            _options: Vec<String>,
        ) -> Result<String> {
            unimplemented!()
        }
    }
}
