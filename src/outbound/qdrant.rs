use crate::domain::ports::VectorDB;
use anyhow::Result;
use async_trait::async_trait;
use qdrant_client::{
    qdrant::{
        Condition, CreateCollectionBuilder, Distance, Filter, PointStruct,
        ScalarQuantizationBuilder, SearchPointsBuilder, UpsertPointsBuilder, VectorParamsBuilder,
    },
    Payload, Qdrant,
};
use serde_json::json;
use std::collections::HashMap;

pub struct QdrantClient {
    client: Qdrant,
}

impl QdrantClient {
    pub fn new(url: &str) -> Result<Self> {
        let client = Qdrant::from_url(url).build()?;
        Ok(Self { client })
    }
}

#[async_trait]
impl VectorDB for QdrantClient {
    async fn create_collection(&self, collection: &str, size: u64) -> Result<bool> {
        self.client
            .create_collection(
                CreateCollectionBuilder::new(collection)
                    .vectors_config(VectorParamsBuilder::new(size, Distance::Cosine))
                    .quantization_config(ScalarQuantizationBuilder::default()),
            )
            .await
            .map(|r| r.result)
            .map_err(anyhow::Error::from)
    }

    async fn delete_collection(&self, collection_name: &str) -> Result<bool> {
        self.client
            .delete_collection(collection_name)
            .await
            .map(|r| r.result)
            .map_err(anyhow::Error::from)
    }

    async fn upsert_points(
        &self,
        collection_name: &str,
        id: u64,
        embedding: Vec<f32>,
        payload: HashMap<String, String>,
    ) -> Result<bool> {
        let payload = json!(payload);

        match Payload::try_from(payload) {
            Ok(payload) => {
                let point = PointStruct::new(id, embedding, payload);
                let request = UpsertPointsBuilder::new(collection_name, vec![point]).build();
                self.client
                    .upsert_points(request)
                    .await
                    .map(|r| r.result.is_some())
                    .map_err(anyhow::Error::from)
            }
            Err(e) => Err(anyhow::Error::from(e)),
        }
    }

    async fn search_points(
        &self,
        collection_name: &str,
        payload_required: HashMap<String, String>,
    ) -> Result<bool> {
        let filter: Vec<Condition> = payload_required
            .iter()
            .map(|(key, value)| Condition::matches(key, value.to_string()))
            .collect();
        self.client
            .search_points(
                SearchPointsBuilder::new(collection_name, vec![0.0; 1536], 1)
                    .filter(Filter::all(filter))
                    .build(),
            )
            .await
            .map(|r| !r.result.is_empty())
            .map_err(anyhow::Error::from)
    }
}
