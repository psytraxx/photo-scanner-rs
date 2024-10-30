use crate::domain::{models::VectorSearchResult, ports::VectorDB};
use anyhow::Result;
use async_trait::async_trait;
use qdrant_client::{
    qdrant::{
        point_id::PointIdOptions, Condition, CreateCollectionBuilder, Distance, Filter,
        PayloadIncludeSelector, PointStruct, ScalarQuantizationBuilder, SearchPointsBuilder,
        UpsertPointsBuilder, VectorParamsBuilder,
    },
    Payload, Qdrant,
};
use serde_json::json;
use std::collections::HashMap;

pub struct QdrantClient {
    client: Qdrant,
    dimensions: u64,
}

impl QdrantClient {
    pub fn new(url: &str, dimensions: u64) -> Result<Self> {
        let client = Qdrant::from_url(url).build()?;
        Ok(Self { client, dimensions })
    }
}

#[async_trait]
impl VectorDB for QdrantClient {
    async fn create_collection(&self, collection: &str) -> Result<bool> {
        self.client
            .create_collection(
                CreateCollectionBuilder::new(collection)
                    .vectors_config(VectorParamsBuilder::new(self.dimensions, Distance::Cosine))
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
        input_vectors: Vec<f32>,
    ) -> Result<Vec<VectorSearchResult>> {
        let filter: Vec<Condition> = payload_required
            .iter()
            .map(|(key, value)| Condition::matches(key, value.to_string()))
            .collect();
        let result = self
            .client
            .search_points(
                SearchPointsBuilder::new(collection_name, input_vectors, 10)
                    .filter(Filter::all(filter))
                    .with_payload(PayloadIncludeSelector::new(vec![
                        "description".into(),
                        "path".into(),
                    ]))
                    .build(),
            )
            .await?;

        let result: Vec<VectorSearchResult> = result
            .result
            .iter()
            .map(|r| {
                let payload: HashMap<String, String> = r
                    .payload
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone().to_string()))
                    .collect();
                let score = r.score;
                let id: u64 = match r.id.as_ref().unwrap().point_id_options {
                    Some(PointIdOptions::Num(id)) => id,
                    _ => panic!("Invalid point id"),
                };
                VectorSearchResult { id, score, payload }
            })
            .collect();
        Ok(result)
    }
}
