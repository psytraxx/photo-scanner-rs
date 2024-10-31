use anyhow::Result;
use async_trait::async_trait;
use std::{collections::HashMap, path::Path, vec::Vec};

#[async_trait]
pub trait Chat: 'static + Send + Sync {
    async fn get_chat(
        &self,
        image_base64: &str,
        persons: &[String],
        folder_name: &Option<String>,
    ) -> Result<String>;

    async fn get_embedding(&self, text: &str) -> Result<Vec<f32>>;
}

pub trait ImageEncoder: 'static + Send + Sync {
    fn resize_and_base64encode_image(&self, image_path: &Path) -> Result<String>;
}

pub trait XMPMetadata: 'static + Send + Sync {
    fn get_xmp_description(&self, path: &Path) -> Result<Option<String>>;
    fn write_xmp_description(&self, text: &str, path: &Path) -> Result<()>;
    fn extract_persons(&self, path: &Path) -> Result<Vec<String>>;
}

#[async_trait]
pub trait VectorDB: 'static + Sync + Send {
    async fn create_collection(&self, collection: &str, size: u64) -> Result<bool>;

    async fn delete_collection(&self, text: &str) -> Result<bool>;

    async fn upsert_points(
        &self,
        collection_name: &str,
        id: u64,
        embedding: Vec<f32>,
        payload: HashMap<String, String>,
    ) -> Result<bool>;

    async fn search_points(
        &self,
        collection_name: &str,
        payload_required: HashMap<String, String>,
    ) -> Result<bool>;
}