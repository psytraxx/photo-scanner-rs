use anyhow::Result;
use async_trait::async_trait;
use std::vec::Vec;

#[async_trait]
pub trait Chat: Sync + Send {
    async fn get_chat(
        &self,
        image_base64: &str,
        persons: &[String],
        folder_name: &Option<String>,
    ) -> Result<String>;

    async fn get_embedding(&self, text: &str) -> Result<Vec<f32>>;
}
