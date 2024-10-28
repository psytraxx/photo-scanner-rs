use anyhow::Result;
use image::open;
use std::{io::Cursor, path::Path};
use tracing::debug;

use base64::{prelude::BASE64_STANDARD, Engine};

use crate::domain::ports::ImageEncoder;

#[derive(Debug, Clone, Default)]
pub struct ImageCrateEncoder;

impl ImageCrateEncoder {
    pub fn new() -> Self {
        Self
    }
}

impl ImageEncoder for ImageCrateEncoder {
    fn resize_and_base64encode_image(&self, file_path: &Path) -> Result<String> {
        // Load the image from the specified file path
        let image = open(file_path)?;

        // Resize the image to 672x672
        let resized_img = image.thumbnail(672, 672);

        // Create a buffer to hold the encoded image
        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);

        resized_img.write_to(&mut cursor, image::ImageFormat::Jpeg)?;

        let image_base64 = BASE64_STANDARD.encode(buffer);
        debug!("{}", image_base64);
        Ok(image_base64)
    }
}
