use anyhow::Result;
use std::{io::Cursor, path::Path};
use tracing::debug;

use base64::{prelude::BASE64_STANDARD, Engine};

pub fn resize_and_base64encode_image(file_path: &Path) -> Result<String> {
    // Load the image from the specified file path
    let image = image::open(file_path)?;

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
