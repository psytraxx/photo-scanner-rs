use crate::domain::{models::VectorSearchResult, ports::VectorDB};
use anyhow::{Error, Result};
use async_trait::async_trait;
use qdrant_client::{
    qdrant::{
        point_id::PointIdOptions, Condition, CreateCollectionBuilder, Distance, Filter,
        GetPointsBuilder, PayloadIncludeSelector, PointId, PointStruct, RetrievedPoint,
        ScalarQuantizationBuilder, ScoredPoint, SearchPointsBuilder, UpsertPointsBuilder,
        VectorParamsBuilder,
    },
    Payload, Qdrant,
};
use serde_json::json;
use std::{collections::HashMap, vec};

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
            .map_err(Error::from)
    }

    async fn delete_collection(&self, collection_name: &str) -> Result<bool> {
        self.client
            .delete_collection(collection_name)
            .await
            .map(|r| r.result)
            .map_err(Error::from)
    }

    async fn upsert_points(
        &self,
        collection_name: &str,
        id: u64,
        embedding: &[f32],
        payload: HashMap<String, String>,
    ) -> Result<bool> {
        let payload = json!(payload);

        match Payload::try_from(payload) {
            Ok(payload) => {
                let point = PointStruct::new(id, embedding.to_vec(), payload);
                let request = UpsertPointsBuilder::new(collection_name, vec![point]).build();
                self.client
                    .upsert_points(request)
                    .await
                    .map(|r| r.result.is_some())
                    .map_err(Error::from)
            }
            Err(e) => Err(Error::from(e)),
        }
    }

    async fn search_points(
        &self,
        collection_name: &str,
        input_vectors: &[f32],
        payload_required: HashMap<String, String>,
    ) -> Result<Vec<VectorSearchResult>> {
        let filter: Vec<Condition> = payload_required
            .iter()
            .map(|(key, value)| Condition::matches(key, value.to_string()))
            .collect();
        let response = self
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

        let result = response.result.iter().map(|r| r.into()).collect();
        Ok(result)
    }

    async fn find_by_id(
        &self,
        collection_name: &str,
        id: &u64,
    ) -> Result<Option<VectorSearchResult>> {
        let query = PointId::from(*id);
        let query = GetPointsBuilder::new(collection_name, vec![query])
            .with_payload(PayloadIncludeSelector::new(vec![
                "description".into(),
                "path".into(),
            ]))
            .build();
        let response = self.client.get_points(query).await?;

        let result: Vec<VectorSearchResult> = response.result.iter().map(|r| r.into()).collect();
        Ok(result.first().cloned())
    }
}

impl From<&ScoredPoint> for VectorSearchResult {
    fn from(point: &ScoredPoint) -> Self {
        let payload = point
            .payload
            .iter()
            .map(|(k, v)| (k.clone(), v.to_string()))
            .collect::<HashMap<String, String>>();

        let score = point.score;

        let id = match point.id.as_ref().unwrap().point_id_options {
            Some(PointIdOptions::Num(id)) => id,
            _ => 0,
        };

        Self { id, score, payload }
    }
}

impl From<&RetrievedPoint> for VectorSearchResult {
    fn from(point: &RetrievedPoint) -> Self {
        let payload: HashMap<String, String> = point
            .payload
            .iter()
            .map(|(k, v)| (k.clone(), v.clone().to_string()))
            .collect();
        let id: u64 = match point.id.as_ref().unwrap().point_id_options {
            Some(PointIdOptions::Num(id)) => id,
            _ => 0,
        };
        Self {
            id,
            score: 1.0, // Not used in this context
            payload,
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_scored_point_to_vector_search_result() {
        let mut payload = HashMap::new();
        payload.insert("test".to_string(), "test".into());

        let payload_len = &payload.len();

        let scored_point = ScoredPoint {
            id: Some(PointId {
                point_id_options: Some(PointIdOptions::Num(123)),
            }),
            score: 0.9,
            payload,
            ..ScoredPoint::default()
        };

        let result: VectorSearchResult = VectorSearchResult::from(&scored_point);

        assert_eq!(result.id, 123);
        assert_eq!(result.score, 0.9);
        assert_eq!(result.payload.len(), *payload_len);
    }

    #[test]
    fn test_retrieved_point_to_vector_search_result() {
        let mut payload = HashMap::new();
        payload.insert("key1".to_string(), "test".into());

        let payload_len = &payload.len();

        let retrieved_point = RetrievedPoint {
            id: Some(PointId {
                point_id_options: Some(PointIdOptions::Num(456)),
            }),
            payload,
            ..RetrievedPoint::default()
        };

        let result: VectorSearchResult = VectorSearchResult::from(&retrieved_point);

        assert_eq!(result.id, 456);
        assert_eq!(result.score, 1.0); // Default score
        assert_eq!(result.payload.len(), *payload_len);
    }
}
