use std::path::Path;

use crate::domain::ports::FileMeta;
use anyhow::Result;
use async_trait::async_trait;
use little_exif::{exif_tag::ExifTag, metadata::Metadata};

#[derive(Debug, Clone)]
pub struct EXIF {}

#[async_trait]
impl FileMeta for EXIF {
    async fn write(&self, text: &str, path: &Path) -> Result<()> {
        let mut metadata = Metadata::new_from_path(path)?;
        let ucs2_bytes: Vec<u8> = text
            .encode_utf16()
            .flat_map(|c| c.to_le_bytes()) // Convert each u16 to a 2-byte little-endian representation
            .collect();
        //https://exiftool.org/TagNames/EXIF.html
        metadata.set_tag(ExifTag::UnknownUNDEF(
            ucs2_bytes,
            0x9c9c,
            little_exif::exif_tag::ExifTagGroup::IFD0,
        ));
        metadata.write_to_file(path)?;
        Ok(())
    }
}
