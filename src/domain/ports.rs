use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Chat: Sync + Send {
    async fn get_chat(
        &self,
        image_base64: String,
        geo_location: Option<String>,
        folder_name: Option<String>,
    ) -> Result<String>;

    async fn get_embedding(&self, text: &str) -> Result<std::vec::Vec<f32>>;
}
