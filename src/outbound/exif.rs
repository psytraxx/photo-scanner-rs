use std::{char::decode_utf16, path::Path};

use anyhow::{anyhow, Result};
use little_exif::{endian::Endian, exif_tag::ExifTag, ifd::ExifTagGroup, metadata::Metadata};
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

    metadata.set_tag(ExifTag::UnknownINT8U(
        string_to_ucs2_little_endian(text),
        XP_COMMENT,
        ExifTagGroup::GENERIC,
    ));

    metadata.write_to_file(path)?;
    Ok(())
}

pub fn get_exif_description(path: &Path) -> Result<Option<String>> {
    let metadata = Metadata::new_from_path(path)?;

    if let Some(tag) = metadata.get_tag_by_hex(XP_COMMENT).next() {
        let comment = ucs2_little_endian_to_string(&tag.value_as_u8_vec(&Endian::Little))?;
        debug!("Tag already exists: {:?}", comment);
        Ok(Some(comment))
    } else {
        debug!("Tag does not exist");
        Ok(None)
    }
}
#[cfg(test)]
mod tests {
    use anyhow::Result;
    use std::{
        fs::{copy, remove_file},
        path::PathBuf,
    };

    use super::*;

    #[test]
    fn test_ucs2_little_endian_to_string() {
        let bytes = vec![0x48, 0x00, 0x65, 0x00, 0x6C, 0x00, 0x6C, 0x00, 0x6F, 0x00]; // "Hello" in UCS-2 LE
        let result = ucs2_little_endian_to_string(&bytes).unwrap();
        assert_eq!(result, "Hello");
    }

    #[test]
    fn test_string_to_ucs2_little_endian() {
        let input = "Hello";
        let result = string_to_ucs2_little_endian(input);
        let expected = vec![0x48, 0x00, 0x65, 0x00, 0x6C, 0x00, 0x6C, 0x00, 0x6F, 0x00]; // "Hello" in UCS-2 LE
        assert_eq!(result, expected);
    }

    #[test]
    fn test_write_and_get_exif_description() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let destination_file_path = temp_dir.path().join("4L2A3805.jpg");

        // Copy an existing JPEG file to the temporary directory
        let source_file = PathBuf::from("testdata/sizilien/4L2A3805.jpg");
        copy(&source_file, &destination_file_path)?;

        let description = "Test Description";
        write_exif_description(description, &destination_file_path).unwrap();

        let result = get_exif_description(&destination_file_path).unwrap();
        assert_eq!(result, Some(description.to_string()));

        // Clean up by deleting the temporary file
        remove_file(&destination_file_path)?;

        Ok(())
    }

    #[test]
    fn test_get_exif_description_none() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let destination_file_path = temp_dir.path().join("4L2A3805.jpg");

        // Copy an existing JPEG file to the temporary directory
        let source_file = PathBuf::from("testdata/sizilien/4L2A3805.jpg");
        copy(&source_file, &destination_file_path)?;

        let result = get_exif_description(&destination_file_path).unwrap();
        assert_eq!(result, None);

        // Clean up by deleting the temporary file
        remove_file(&destination_file_path)?;

        Ok(())
    }

    #[test]
    fn test_invalid_ucs2_little_endian_to_string() {
        let bytes = vec![0x48, 0x00, 0x65]; // Invalid UCS-2 LE
        let result = ucs2_little_endian_to_string(&bytes);
        assert!(result.is_err());
    }
}
