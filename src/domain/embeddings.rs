use super::{
    file_utils::list_jpeg_files,
    ports::{Chat, VectorDB, XMPMetadata},
};
use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};
use std::{
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    path::PathBuf,
    sync::Arc,
};
use tracing::{error, info, warn};

// Maximum number of chunks for embeddings API
const CHUNK_SIZE: usize = 10;
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
            self.vector_db.create_collection(COLLECTION_NAME).await?;
        }

        // Create a progress bar with the total length of the vector.
        let progress_bar = Arc::new(ProgressBar::new(files_list.len() as u64));
        progress_bar.set_style(
            ProgressStyle::default_bar()
                .template("Processing [{elapsed_precise}] [{wide_bar}] {pos}/{len} ({eta})")?,
        );

        for chunk in files_list.chunks(CHUNK_SIZE) {
            let progress_bar = Arc::clone(&progress_bar);

            progress_bar.inc(chunk.len() as u64);

            if let Err(e) = self.process_paths(chunk).await {
                error!("Error processing chunk {:?}: {}", chunk, e);
            }
        }
        Ok(())
    }

    async fn process_paths(&self, paths: &[PathBuf]) -> Result<()> {
        struct EmbeddingTask {
            id: u64,
            description: String,
            path: PathBuf,
        }

        let mut tasks = Vec::new();

        for path in paths {
            if let Some(description) = self.xmp_metadata.get_description(path)? {
                let mut hasher = DefaultHasher::new();
                path.hash(&mut hasher);
                let id = hasher.finish();

                if let Some(existing_entry) =
                    self.vector_db.find_by_id(COLLECTION_NAME, &id).await?
                {
                    if let Some(existing_description) = existing_entry.payload.get("description") {
                        if existing_description.contains(&description) {
                            info!(
                                "Skipping {} because of existing ID with same description",
                                path.display()
                            );
                            continue;
                        }
                    }
                }

                tasks.push(EmbeddingTask {
                    id,
                    description,
                    path: path.clone(),
                });
            } else {
                warn!("Skipping {} because of missing description", path.display());
            }
        }

        if !tasks.is_empty() {
            let descriptions: Vec<String> = tasks
                .iter()
                .map(|result| result.description.clone())
                .collect();
            let embeddings = self.chat.get_embeddings(descriptions).await?;

            for (result, embedding) in tasks.iter().zip(embeddings.iter()) {
                let message = result
                    .path
                    .parent()
                    .unwrap()
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap();

                let mut payload = HashMap::new();
                payload.insert("path".to_string(), result.path.display().to_string());
                payload.insert("description".to_string(), result.description.clone());
                payload.insert("folder".to_string(), message.into());

                info!(
                    "Processing {}: {:?} {:?}, {}",
                    result.path.display(),
                    &payload,
                    embedding.len(),
                    result.id
                );

                self.vector_db
                    .upsert_points(COLLECTION_NAME, result.id, embedding, payload)
                    .await?;
            }

            return Ok(());
        }

        Ok(())
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

    use crate::domain::models::VectorSearchResult;
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
        async fn get_image_description(
            &self,
            _image_base64: &str,
            _persons: &[String],
            _folder_name: &Option<String>,
        ) -> Result<String> {
            unimplemented!()
        }

        async fn get_embeddings(&self, _texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
            let mut rng = rand::thread_rng();
            let embedding: Vec<f32> = (0..1536).map(|_| rng.gen()).collect();
            Ok(vec![embedding])
        }

        async fn process_search_result(
            &self,
            _question: &str,
            _options: &[String],
        ) -> Result<String> {
            unimplemented!()
        }
    }

    struct VectorDBMock {
        store_embeddings: Mutex<HashMap<String, Vec<VectorDBEntry>>>,
    }

    #[derive(Clone, Debug)]
    struct VectorDBEntry {
        id: u64,
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
        async fn create_collection(&self, _collection: &str) -> Result<bool> {
            unimplemented!()
        }

        async fn delete_collection(&self, _text: &str) -> Result<bool> {
            unimplemented!()
        }

        async fn find_by_id(
            &self,
            _collection_name: &str,
            _id: &u64,
        ) -> Result<Option<VectorSearchResult>> {
            return Ok(None);
        }

        async fn upsert_points(
            &self,
            collection_name: &str,
            id: u64,
            _embedding: &[f32],
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
            collection.push(VectorDBEntry { id, payload });
            Ok(true)
        }

        async fn search_points(
            &self,
            collection_name: &str,
            _input_vectors: &[f32],
            _payload_required: HashMap<String, String>,
        ) -> Result<Vec<VectorSearchResult>> {
            let entries = self.store_embeddings.lock().unwrap();
            match entries.get(collection_name) {
                Some(entries) => {
                    debug!(
                        "Found {:?} entries in collection {}",
                        entries, collection_name
                    );

                    entries
                        .iter()
                        .map(|entry| {
                            Ok(VectorSearchResult {
                                id: entry.id,
                                score: 0.0,
                                payload: entry.payload.clone(),
                            })
                        })
                        .collect()
                }
                None => return Ok(Vec::new()),
            }
        }
    }
}
