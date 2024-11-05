use super::models::{VectorInput, VectorOutput};
use anyhow::Result;
use async_trait::async_trait;
use std::{collections::HashMap, path::Path, vec::Vec};

#[async_trait]
pub trait Chat: 'static + Send + Sync {
    async fn get_image_description(
        &self,
        image_base64: &str,
        persons: &[String],
        folder_name: &Option<String>,
    ) -> Result<String>;

    async fn get_embeddings(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>>;

    async fn process_search_result(&self, question: &str, options: &[String]) -> Result<String>;
}

pub trait ImageEncoder: 'static + Send + Sync {
    fn resize_and_base64encode_image(&self, image_path: &Path) -> Result<String>;
}

pub trait XMPMetadata: 'static + Send + Sync {
    fn get_description(&self, path: &Path) -> Result<Option<String>>;
    fn get_geolocation(&self, path: &Path) -> Result<Option<String>>;
    fn set_description(&self, text: &str, path: &Path) -> Result<()>;
    fn get_persons(&self, path: &Path) -> Result<Vec<String>>;
}

#[async_trait]
pub trait VectorDB: 'static + Sync + Send {
    async fn create_collection(&self, collection: &str) -> Result<bool>;

    async fn delete_collection(&self, text: &str) -> Result<bool>;

    async fn upsert_points(&self, collection_name: &str, inputs: &[VectorInput]) -> Result<bool>;

    async fn search_points(
        &self,
        collection_name: &str,
        input_vectors: &[f32],
        payload_required: HashMap<String, String>,
    ) -> Result<Vec<VectorOutput>>;

    async fn find_by_id(&self, collection_name: &str, id: &u64) -> Result<Option<VectorOutput>>;
}
