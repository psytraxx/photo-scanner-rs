use anyhow::Result;
use async_trait::async_trait;
use std::{path::Path, vec::Vec};

#[async_trait]
pub trait Chat: Sync + Send {
    async fn get_chat(
        &self,
        image_base64: String,
        geo_location: Option<String>,
        folder_name: Option<String>,
    ) -> Result<String>;

    async fn get_embedding(&self, text: &str) -> Result<Vec<f32>>;
}

#[async_trait]
pub trait FileMeta: Sync + Send {
    async fn write(&self, text: &str, path: &Path) -> Result<()>;
}
