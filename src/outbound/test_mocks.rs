#[cfg(test)]
pub mod tests {
    use std::{collections::HashMap, sync::Mutex};

    use anyhow::Result;
    use async_trait::async_trait;
    use rand::Rng;
    use tracing::debug;

    use crate::domain::{
        models::{VectorInput, VectorOutput},
        ports::{Chat, VectorDB},
    };

    #[derive(Clone, Debug)]
    pub struct ChatMock;

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

    #[derive(Default)]
    pub struct VectorDBMock {
        store_embeddings: Mutex<HashMap<String, Vec<VectorInput>>>,
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
        async fn create_collection(&self, collection_name: &str) -> Result<bool> {
            let mut store = self.store_embeddings.lock().unwrap();
            if !store.contains_key(collection_name) {
                store.insert(collection_name.to_string(), Vec::new());
            }
            Ok(true)
        }

        async fn delete_collection(&self, collection_name: &str) -> Result<bool> {
            let mut store = self.store_embeddings.lock().unwrap();
            let result = store.remove(collection_name);
            Ok(result.is_some())
        }

        async fn find_by_id(
            &self,
            collection_name: &str,
            id: &u64,
        ) -> Result<Option<VectorOutput>> {
            let store = self.store_embeddings.lock().unwrap();
            let collection = store
                .get(collection_name)
                .expect("Collection missing in store");

            let result = collection
                .iter()
                .find(|v| v.id == *id)
                .map(|v| VectorOutput {
                    id: v.id,
                    score: None,
                    payload: v.payload.clone(),
                });
            Ok(result)
        }

        async fn upsert_points(
            &self,
            collection_name: &str,
            inputs: &[VectorInput],
        ) -> Result<bool> {
            let mut store = self.store_embeddings.lock().unwrap();
            let collection = store.get_mut(collection_name).unwrap();

            inputs.iter().for_each(|input| {
                // Find and remove an existing entry with the same ID
                if collection.iter().any(|entry| entry.id == input.id) {
                    collection.retain(|entry| entry.id != input.id);
                }

                // Insert a new entry
                collection.push(VectorInput::new(
                    input.id,
                    input.embedding.clone(),
                    input.payload.clone(),
                ));
            });
            Ok(true)
        }

        async fn search_points(
            &self,
            collection_name: &str,
            _input_vectors: &[f32],
            _payload_required: HashMap<String, String>,
        ) -> Result<Vec<VectorOutput>> {
            let store = self.store_embeddings.lock().unwrap();
            match store.get(collection_name) {
                Some(entries) => {
                    debug!(
                        "Found {:?} entries in collection {}",
                        entries, collection_name
                    );

                    entries
                        .iter()
                        .map(|entry| {
                            Ok(VectorOutput {
                                id: entry.id,
                                score: None,
                                payload: entry.payload.clone(),
                            })
                        })
                        .collect()
                }
                None => return Ok(Vec::new()),
            }
        }
    }

    #[tokio::test]
    async fn test_chat_mock() {
        let chat_mock = ChatMock;

        // Test get_image_description
        let description = chat_mock
            .get_image_description("image_base64", &[], &None)
            .await
            .unwrap();
        assert_eq!(description, "description");

        // Test get_embeddings
        let embeddings = chat_mock
            .get_embeddings(vec!["test".to_string()])
            .await
            .unwrap();
        assert_eq!(embeddings[0].len(), 1536);
    }

    #[tokio::test]
    async fn test_vector_db_mock() {
        let vector_db_mock = VectorDBMock::new();

        let id = 1;
        let collection = "test";

        // Create collection
        let created = vector_db_mock.create_collection(collection).await.unwrap();
        assert!(created);

        // Test upsert_points
        let mut input = VectorInput::new(id, vec![0.1, 0.2, 0.3], HashMap::new());
        let inserted = vector_db_mock
            .upsert_points("test", &[input.clone()])
            .await
            .unwrap();
        assert!(inserted);

        // Test search_points
        let outputs = vector_db_mock
            .search_points("test", &[0.1, 0.2, 0.3], HashMap::new())
            .await
            .unwrap();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].id, id);

        // Test upsert_points with existing ID
        let payload = HashMap::from([("key".to_string(), "value".to_string())]);
        input.payload = payload;

        let upserted = vector_db_mock
            .upsert_points("test", &[input])
            .await
            .unwrap();
        assert!(upserted);

        // Test get point

        let point = vector_db_mock.find_by_id("test", &id).await.unwrap();

        assert!(point.is_some());
    }
}
