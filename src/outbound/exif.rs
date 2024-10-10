use std::{char::decode_utf16, path::Path};

use anyhow::{anyhow, Result};
use little_exif::{endian::Endian, exif_tag::ExifTag, metadata::Metadata};
use tracing::debug;

const XP_COMMENT: u16 = 0x9C9C;

/// Converts a byte slice in UCS-2 little-endian format to a String.
fn ucs2_little_endian_to_string(bytes: &[u8]) -> Result<String> {
    if bytes.len() % 2 != 0 {
        return Err(anyhow!("Invalid byte array length for UCS-2"));
    }

    let u16_data: Vec<u16> = bytes
        .chunks(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();

    decode_utf16(u16_data)
        .map(|r| r.map_err(|e| format!("Invalid UTF-16 code unit: {}", e)))
        .collect::<Result<String, _>>()
        .map_err(|e| anyhow!(e.to_string()))
}

/// Converts a string to a byte vector in UCS-2 little-endian format.
fn string_to_ucs2_little_endian(input: &str) -> Vec<u8> {
    input
        .encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect()
}

pub fn write_exif_description(text: &str, path: &Path) -> Result<()> {
    let mut metadata = Metadata::new_from_path(path)?;

    match metadata.get_tag_by_hex(XP_COMMENT) {
        Some(tag) => {
            let comment = ucs2_little_endian_to_string(&tag.value_as_u8_vec(&Endian::Little));
            debug!("Tag already exists: {:?}", comment);
        }
        None => {
            debug!("Tag does not exist");
        }
    }

    metadata.set_tag(ExifTag::UnknownINT8U(
        string_to_ucs2_little_endian(text),
        XP_COMMENT,
        little_exif::exif_tag::ExifTagGroup::IFD0,
    ));

    metadata.write_to_file(path)?;
    Ok(())
}
