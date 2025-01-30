use crate::domain::{
    models::{VectorInput, VectorOutput, VectorOutputList},
    ports::VectorDB,
};
use anyhow::{Error, Result};
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
use std::{collections::HashMap, env::var, vec};

pub struct QdrantClient {
    client: Qdrant,
    dimensions: u64,
}

impl QdrantClient {
    pub fn new() -> Result<Self> {
        // load env from .env file
        dotenv::dotenv().ok();
        let url = var("QDRANT_GRPC_URL").expect("QDRANT_GRPC must be set in .env file");
        let dimensions: u64 = var("QDRANT_GRPC_DIMENSION")
            .expect("QDRANT_GRPC_DIMENSION must be set in .env file")
            .parse()
            .expect("QDRANT_GRPC_DIMENSION must be a valid u64");

        let client = Qdrant::from_url(&url).build()?;
        Ok(Self { client, dimensions })
    }
}

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

    async fn upsert_points(&self, collection_name: &str, inputs: &[VectorInput]) -> Result<bool> {
        let points: Result<Vec<_>> = inputs
            .iter()
            .map(|i| {
                let payload = json!(i.payload);
                Payload::try_from(payload)
                    .map(|payload| PointStruct::new(i.id, i.embedding.clone(), payload))
                    .map_err(Error::from)
            })
            .collect();

        let points = points?;

        let request = UpsertPointsBuilder::new(collection_name, points).build();
        self.client
            .upsert_points(request)
            .await
            .map(|r| r.result.is_some())
            .map_err(Error::from)
    }

    async fn search_points(
        &self,
        collection_name: &str,
        input_vectors: &[f32],
        payload_required: HashMap<String, String>,
    ) -> Result<VectorOutputList> {
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

    async fn find_by_id(&self, collection_name: &str, id: &u64) -> Result<Option<VectorOutput>> {
        let query = PointId::from(*id);
        let query = GetPointsBuilder::new(collection_name, vec![query])
            .with_payload(PayloadIncludeSelector::new(vec![
                "description".into(),
                "path".into(),
            ]))
            .build();
        let response = self.client.get_points(query).await?;

        let result: Vec<VectorOutput> = response.result.iter().map(|r| r.into()).collect();
        Ok(result.first().cloned())
    }
}

impl From<&ScoredPoint> for VectorOutput {
    fn from(point: &ScoredPoint) -> Self {
        let payload = point
            .payload
            .iter()
            .map(|(k, v)| (k.clone(), v.to_string()))
            .collect::<HashMap<String, String>>();

        let score = point.score;

        let point_id = point.id.as_ref();

        let id = match point_id.and_then(|id| id.point_id_options.clone()) {
            Some(PointIdOptions::Num(id)) => id,
            _ => 0,
        };

        Self {
            id,
            score: Some(score),
            payload,
        }
    }
}

impl From<&RetrievedPoint> for VectorOutput {
    fn from(point: &RetrievedPoint) -> Self {
        let payload: HashMap<_, _> = point
            .payload
            .iter()
            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
            .collect();

        let id = point
            .id
            .as_ref()
            .and_then(|id| match id.point_id_options {
                Some(PointIdOptions::Num(id)) => Some(id),
                _ => None,
            })
            .unwrap_or_default();

        Self {
            id,
            score: None, // Not used in this context
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

        let result = VectorOutput::from(&scored_point);

        assert_eq!(result.id, 123);
        assert_eq!(result.score, Some(0.9));
        assert_eq!(result.payload.len(), *payload_len);
    }

    #[test]
    fn test_scored_point_to_vector_search_result_no_id() {
        let mut payload = HashMap::new();
        payload.insert("key1".to_string(), "test".into());

        let payload_len = &payload.len();

        let retrieved_point = ScoredPoint {
            id: None,
            payload,
            score: 0.9,
            ..ScoredPoint::default()
        };

        let result = VectorOutput::from(&retrieved_point);

        assert_eq!(result.id, 0);
        assert_eq!(result.score, Some(0.9));
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

        let result = VectorOutput::from(&retrieved_point);

        assert_eq!(result.id, 456);
        assert_eq!(result.score, None);
        assert_eq!(result.payload.len(), *payload_len);
    }

    #[test]
    fn test_retrieved_point_to_vector_search_result_no_id() {
        let mut payload = HashMap::new();
        payload.insert("key1".to_string(), "test".into());

        let payload_len = &payload.len();

        let retrieved_point = RetrievedPoint {
            id: None,
            payload,
            ..RetrievedPoint::default()
        };

        let result = VectorOutput::from(&retrieved_point);

        assert_eq!(result.id, 0);
        assert_eq!(result.score, None);
        assert_eq!(result.payload.len(), *payload_len);
    }
}
