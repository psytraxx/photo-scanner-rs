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

pub struct EmbeddingsService<C, V, X>
where
    C: Chat,
    V: VectorDB,
    X: XMPMetadata,
{
    chat: Arc<C>,
    xmp_metadata: Arc<X>,
    vector_db: Arc<V>,
}

impl<C, V, X> EmbeddingsService<C, V, X>
where
    C: Chat,
    V: VectorDB,
    X: XMPMetadata,
{
    pub fn new(chat: Arc<C>, xmp_metadata: Arc<X>, vector_db: Arc<V>) -> Self {
        EmbeddingsService {
            chat,
            xmp_metadata,
            vector_db,
        }
    }

    pub async fn create_collection(&self) -> Result<()> {
        self.vector_db.delete_collection(COLLECTION_NAME).await?;
        self.vector_db.create_collection(COLLECTION_NAME).await?;
        Ok(())
    }

    pub async fn generate(&self, root_path: &PathBuf) -> Result<()> {
        let files_list = list_jpeg_files(root_path)?;

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
            let id = generate_hash(&path);

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

                VectorInput::new(task.id, embedding, payload)
            })
            .collect();

        // Upsert the data into the vector database
        self.vector_db
            .upsert_points(COLLECTION_NAME, &inputs)
            .await?;

        Ok(())
    }
}

fn generate_hash(path: &PathBuf) -> u64 {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
pub mod tests {
    use crate::domain::ports::VectorDB;
    use crate::{
        domain::{
            embeddings::{generate_hash, EmbeddingsService, COLLECTION_NAME},
            models::VectorInput,
        },
        outbound::{
            test_mocks::tests::{ChatMock, VectorDBMock},
            xmp::XMPToolkitMetadata,
        },
    };
    use anyhow::Result;
    use std::collections::HashMap;
    use std::{
        fs::{copy, remove_file},
        path::PathBuf,
        sync::Arc,
    };
    #[tokio::test]
    async fn test_generate_embeddings() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;

        let destination_file_path1 = temp_dir.path().join("example-full.jpg");
        // Copy an existing JPEG with NO description to the temporary directory - this file should be skipped
        let source_file = PathBuf::from("testdata/example-full.jpg");
        copy(&source_file, &destination_file_path1)?;

        let destination_file_path2 = temp_dir.path().join("example-existing-description-xmp.jpg");
        // Copy an existing JPEG wit an existing description to the temporary directory
        let source_file = PathBuf::from("testdata/example-existing-description-xmp.jpg");
        copy(&source_file, &destination_file_path2)?;

        // Initialize dependencies
        let chat = Arc::new(ChatMock);
        let xmp_metadata = Arc::new(XMPToolkitMetadata::new());
        let vector_db = Arc::new(VectorDBMock::new());
        vector_db.create_collection(COLLECTION_NAME).await?;

        // Create the DescriptionService instance
        let service = EmbeddingsService::new(chat, xmp_metadata.clone(), vector_db);

        // Generate descriptions for the files in the temporary directory
        let result = service.generate(&temp_dir.path().into()).await;

        assert!(result.is_ok());

        // Clean up by deleting the temporary file(s)
        remove_file(&destination_file_path1)?;
        remove_file(&destination_file_path2)?;

        Ok(())
    }

    #[tokio::test]
    async fn test_generate_embeddings_existing() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;

        let destination_file_path2 = temp_dir.path().join("example-existing-description-xmp.jpg");
        // Copy an existing JPEG wit an existing description to the temporary directory
        let source_file = PathBuf::from("testdata/example-existing-description-xmp.jpg");
        copy(&source_file, &destination_file_path2)?;

        // Initialize dependencies
        let chat = Arc::new(ChatMock);
        let xmp_metadata = Arc::new(XMPToolkitMetadata::new());
        let vector_db = Arc::new(VectorDBMock::new());

        let id_path2 = generate_hash(&destination_file_path2);

        let input = vec![VectorInput::new(
            id_path2,
            vec![0.1, 0.2, 0.3],
            HashMap::from([(
                "description".to_string(),
                "Existing description".to_string(), // has to be inside the referenced file
            )]),
        )];

        vector_db.create_collection(COLLECTION_NAME).await?;
        vector_db.upsert_points(COLLECTION_NAME, &input).await?;

        // Create the DescriptionService instance
        let service = EmbeddingsService::new(chat, xmp_metadata.clone(), vector_db);

        // Generate descriptions for the files in the temporary directory
        let result = service.generate(&temp_dir.path().into()).await;

        assert!(result.is_ok());

        // Clean up by deleting the temporary file(s)
        remove_file(&destination_file_path2)?;

        Ok(())
    }

    #[test]
    fn test_generate_hash() {
        // Test case 1: Same path should generate same hash
        let path1 = PathBuf::from("/test/path/file.jpg");
        let path2 = PathBuf::from("/test/path/file.jpg");
        assert_eq!(generate_hash(&path1), generate_hash(&path2));

        // Test case 2: Different paths should generate different hashes
        let path3 = PathBuf::from("/test/path/file2.jpg");
        assert_ne!(generate_hash(&path1), generate_hash(&path3));

        // Test case 3: Empty path
        let empty_path = PathBuf::from("");
        let result = generate_hash(&empty_path);
        assert!(result > 0); // Hash should still be generated

        // Test case 4: Case sensitivity
        let path_lower = PathBuf::from("/test/path/FILE.jpg");
        let path_upper = PathBuf::from("/test/path/file.jpg");
        assert_ne!(generate_hash(&path_lower), generate_hash(&path_upper));

        // Test case 5: Path with special characters
        let path_special = PathBuf::from("/test/path/file#1@.jpg");
        let result = generate_hash(&path_special);
        assert!(result > 0);

        // Test case 6: Hash should be always the same
        let path1 = PathBuf::from("/test/path/file.jpg");
        assert_eq!(12776033237478848503, generate_hash(&path1));
    }
}
