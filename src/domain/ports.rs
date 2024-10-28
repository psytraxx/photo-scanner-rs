use anyhow::Result;
use async_trait::async_trait;
use std::{path::Path, vec::Vec};

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
