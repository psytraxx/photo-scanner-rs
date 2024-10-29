use super::{
    file_utils::list_jpeg_files,
    ports::{Chat, VectorDB, XMPMetadata},
};
use anyhow::Result;
use futures::{stream::iter, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    path::PathBuf,
    sync::Arc,
};
use tracing::{error, warn};

// Maximum number of concurrent tasks for embeddings API
const MAX_CONCURRENT_TASKS: usize = 10;
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
        // Traverse the files and process them with limited concurrency.
        let files_list = list_jpeg_files(root_path)?;

        if DROP_COLLECTION {
            self.vector_db.delete_collection(COLLECTION_NAME).await?;
            self.vector_db
                .create_collection(COLLECTION_NAME, 1536)
                .await?;
        }

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
                async move {
                    progress_bar.inc(1);

                    if let Err(e) = self.process_path(&path).await {
                        error!("Error processing path {}: {}", path.display(), e);
                    }
                }
            })
            .await;
        Ok(())
    }

    async fn process_path(&self, path: &PathBuf) -> Result<()> {
        // Extract persons from the image, handling any errors.
        match self.xmp_metadata.get_xmp_description(path) {
            Ok(Some(description)) => {
                match self.chat.get_embedding(&description).await {
                    Ok(embedding) => {
                        let mut payload = HashMap::new();
                        payload.insert("path".to_string(), path.display().to_string());
                        payload.insert("description".to_string(), description);
                        let mut hasher = DefaultHasher::new();
                        path.hash(&mut hasher);
                        let id = hasher.finish();
                        match self
                            .vector_db
                            .upsert_points(COLLECTION_NAME, id, embedding, payload)
                            .await
                        {
                            Ok(_) => (),
                            Err(e) => {
                                error!("Error upserting points: {}", e);
                                return Err(e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error getting embedding: {}", e);
                        return Err(e);
                    }
                }

                Ok(())
            }
            Ok(None) => {
                warn!(
                    "Skipping {} because of missing XMP descripton ",
                    path.display()
                );
                Ok(())
            }
            Err(e) => {
                error!("Error while processing {}: {}", path.display(), e);
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use async_trait::async_trait;
    use rand::Rng;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::{
        fs::{copy, remove_file},
        path::PathBuf,
        sync::Arc,
    };
    use tracing::debug;

    use crate::{
        domain::{
            embeddings::EmbeddingsService,
            ports::{Chat, VectorDB},
        },
        outbound::xmp::XMPToolkitMetadata,
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

        // Create the DescriptionService instance
        let service = EmbeddingsService::new(chat, xmp_metadata.clone(), vector_db);

        // Generate descriptions for the files in the temporary directory
        let result = service.generate(&temp_dir.path().into()).await;

        assert!(result.is_ok());

        // Clean up by deleting the temporary file
        remove_file(&destination_file_path)?;

        Ok(())
    }

    struct ChatMock;

    #[async_trait]
    impl Chat for ChatMock {
        async fn get_chat(
            &self,
            _image_base64: &str,
            _persons: &[String],
            _folder_name: &Option<String>,
        ) -> Result<String> {
            unimplemented!()
        }

        // Mock implementation for get_embedding
        async fn get_embedding(&self, _text: &str) -> Result<Vec<f32>> {
            let mut rng = rand::thread_rng();
            let embedding: Vec<f32> = (0..1536).map(|_| rng.gen()).collect();
            Ok(embedding)
        }
    }

    struct VectorDBMock {
        store_embeddings: Mutex<HashMap<String, Vec<VectorDBEntry>>>,
    }

    #[derive(Clone, Debug)]
    struct VectorDBEntry {
        id: u64,
        embedding: Vec<f32>,
        payload: HashMap<String, String>,
    }

    impl VectorDBMock {
        pub fn new() -> Self {
            Self {
                store_embeddings: Mutex::new(HashMap::new()),
            }
        }
    }

    #[async_trait]
    impl VectorDB for VectorDBMock {
        async fn create_collection(&self, _collection: &str, _size: u64) -> Result<bool> {
            unimplemented!()
        }

        async fn delete_collection(&self, _text: &str) -> Result<bool> {
            unimplemented!()
        }

        async fn upsert_points(
            &self,
            collection_name: &str,
            id: u64,
            embedding: Vec<f32>,
            payload: HashMap<String, String>,
        ) -> Result<bool> {
            let mut entries = self.store_embeddings.lock().unwrap();
            if !entries.contains_key(collection_name) {
                entries.insert(collection_name.to_string(), Vec::new());
            }

            let collection = entries.get_mut(collection_name).unwrap();

            // Find and remove an existing entry with the same ID
            if collection.iter().any(|entry| entry.id == id) {
                collection.retain(|entry| entry.id != id);
            }

            // Insert a new entry
            collection.push(VectorDBEntry {
                id,
                embedding,
                payload,
            });
            Ok(true)
        }

        async fn search_points(
            &self,
            collection_name: &str,
            _payload_required: HashMap<String, String>,
        ) -> Result<bool> {
            let entries = self.store_embeddings.lock().unwrap();
            match entries.get(collection_name) {
                Some(entries) => {
                    debug!(
                        "Found {:?} entries in collection {}",
                        entries, collection_name
                    );
                    return Ok(!entries.is_empty());
                }
                None => return Ok(false),
            }
        }
    }
}
