use super::{
    file_utils::list_jpeg_files,
    ports::{Chat, VectorDB, XMPMetadata},
};
use crate::domain::models::VectorInput;
use anyhow::Result;
use futures::stream::{iter, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use tokio::time::sleep;
use tracing::{error, info, warn};

// Maximum number of chunks for embeddings API
const CHUNK_SIZE: usize = 25;
const COLLECTION_NAME: &str = "photos";
const DROP_COLLECTION: bool = false;

pub struct EmbeddingsService {
    chat: Arc<dyn Chat>,
    xmp_metadata: Arc<dyn XMPMetadata>,
    vector_db: Arc<dyn VectorDB>,
}

impl EmbeddingsService {
    pub fn new(
        chat: Arc<dyn Chat>,
        xmp_metadata: Arc<dyn XMPMetadata>,
        vector_db: Arc<dyn VectorDB>,
    ) -> Self {
        EmbeddingsService {
            chat,
            xmp_metadata,
            vector_db,
        }
    }

    pub async fn generate(&self, root_path: &PathBuf) -> Result<()> {
        let files_list = list_jpeg_files(root_path)?;

        if DROP_COLLECTION {
            self.vector_db.delete_collection(COLLECTION_NAME).await?;
            self.vector_db.create_collection(COLLECTION_NAME).await?;
        }

        let progress_bar = Arc::new(ProgressBar::new(files_list.len() as u64));
        progress_bar.set_style(
            ProgressStyle::default_bar()
                .template("Processing [{elapsed_precise}] [{wide_bar}] {pos}/{len} ({eta})")
                .expect("Invalid progress bar style"),
        );

        let chunks = files_list.chunks(CHUNK_SIZE);

        for chunk in chunks {
            progress_bar.inc(chunk.len() as u64);
            if let Err(e) = self.process_paths(chunk.to_vec()).await {
                error!("Error processing chunk: {}", e);
            }
        }

        progress_bar.finish();
        Ok(())
    }

    async fn process_paths(&self, paths: Vec<PathBuf>) -> Result<()> {
        #[derive(Debug)]
        struct EmbeddingTask {
            id: u64,
            description: String,
            path: PathBuf,
        }

        let path_futures = paths.into_iter().map(|path| async move {
            // Try to retrieve the description from the XMP metadata
            let description = match self.xmp_metadata.get_description(&path) {
                Ok(Some(description)) => description,
                _ => {
                    warn!(
                        "Skipping {}: missing or failed to get description",
                        path.display()
                    );
                    return None;
                }
            };

            // Generate a unique ID for the path
            let mut hasher = DefaultHasher::new();
            path.hash(&mut hasher);
            let id = hasher.finish();

            // Check for existing entry in the vector database
            if let Ok(Some(existing_entry)) = self.vector_db.find_by_id(COLLECTION_NAME, &id).await
            {
                if let Some(existing_description) = existing_entry.payload.get("description") {
                    if existing_description.contains(&description) {
                        // Skip if the description matches
                        info!(
                            "Skipping {}: existing ID with the same description",
                            path.display()
                        );
                        return None;
                    }
                }
            }

            // No match found, create and return the task
            Some(EmbeddingTask {
                id,
                description,
                path,
            })
        });

        // process fututres and collect tasks
        let embedding_tasks: Vec<EmbeddingTask> = iter(path_futures)
            .buffer_unordered(CHUNK_SIZE)
            .filter_map(|task| async { task })
            .collect::<Vec<_>>()
            .await;

        if embedding_tasks.is_empty() {
            return Ok(());
        }

        // to avoid rate limiting, sleep for a while
        sleep(Duration::from_millis(100)).await;

        let descriptions: Vec<_> = embedding_tasks
            .iter()
            .map(|task| task.description.clone())
            .collect();
        let embeddings = self.chat.get_embeddings(descriptions).await?;

        let inputs: Vec<VectorInput> = embedding_tasks
            .into_iter()
            .zip(embeddings.into_iter())
            .map(|(task, embedding)| {
                let folder_name = task
                    .path
                    .parent()
                    .and_then(|parent| parent.file_name())
                    .and_then(|name| name.to_str())
                    .unwrap_or("Unknown")
                    .to_string();

                let payload = HashMap::from([
                    ("path".to_string(), task.path.display().to_string()),
                    ("description".to_string(), task.description.clone()),
                    ("folder".to_string(), folder_name),
                ]);

                VectorInput {
                    id: task.id,
                    embedding,
                    payload,
                }
            })
            .collect();

        // Upsert the data into the vector database
        self.vector_db
            .upsert_points(COLLECTION_NAME, &inputs)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use crate::{
        domain::{embeddings::EmbeddingsService, ports::VectorDB},
        outbound::{
            test_mocks::tests::{ChatMock, VectorDBMock},
            xmp::XMPToolkitMetadata,
        },
    };
    use anyhow::Result;
    use std::{
        fs::{copy, remove_file},
        path::PathBuf,
        sync::Arc,
    };
    #[tokio::test]
    async fn test_generate_embeddings() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let destination_file_path = temp_dir.path().join("4L2A3805.jpg");

        // Copy an existing JPEG file to the temporary directory
        let source_file = PathBuf::from("testdata/picasa/PXL_20230408_060152625.jpg");
        copy(&source_file, &destination_file_path)?;

        // Initialize dependencies
        let chat = Arc::new(ChatMock);
        let xmp_metadata = Arc::new(XMPToolkitMetadata::new());

        let vector_db = Arc::new(VectorDBMock::new());
        vector_db.create_collection("photos").await?;
        // Create the DescriptionService instance
        let service = EmbeddingsService::new(chat, xmp_metadata.clone(), vector_db);

        // Generate descriptions for the files in the temporary directory
        let result = service.generate(&temp_dir.path().into()).await;

        assert!(result.is_ok());

        // Clean up by deleting the temporary file
        remove_file(&destination_file_path)?;

        Ok(())
    }
}
