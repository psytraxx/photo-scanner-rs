use anyhow::{anyhow, Result};
use little_exif::{exif_tag::ExifTag, ifd::ExifTagGroup, metadata::Metadata};
use std::{char::decode_utf16, path::Path};
use tracing::debug;

const XP_COMMENT: u16 = 0x9C9C;

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
    let endian = metadata.get_endian();
    if let Some(tag) = metadata
        .get_tag_by_hex(XP_COMMENT, Some(ExifTagGroup::GENERIC))
        .next()
    {
        let comment = ucs2_little_endian_to_string(&tag.value_as_u8_vec(&endian))?;
        debug!("Tag exists: {:?}", comment);
        Ok(Some(comment))
    } else {
        debug!("Tag does not exist");
        Ok(None)
    }
}

pub fn get_exif_location(path: &Path) -> Result<Option<String>> {
    let metadata = Metadata::new_from_path(path)?;
    let endian = metadata.get_endian();
    let latitude = match metadata
        .get_tag_by_hex(
            ExifTag::GPSLatitude(Vec::new()).as_u16(),
            Some(ExifTagGroup::GPS),
        )
        .next()
    {
        Some(tag) => {
            let bytes = tag.value_as_u8_vec(&endian);
            bytes_to_geolocation(&bytes).ok()
        }
        None => {
            debug!("Tag does not exist");
            None
        }
    };
    let longitude: Option<f64> = match metadata
        .get_tag_by_hex(
            ExifTag::GPSLongitude(Vec::new()).as_u16(),
            Some(ExifTagGroup::GPS),
        )
        .next()
    {
        Some(tag) => {
            let bytes = tag.value_as_u8_vec(&endian);
            bytes_to_geolocation(&bytes).ok()
        }
        None => {
            debug!("Tag does not exist");
            None
        }
    };
    if latitude.is_some() && longitude.is_some() {
        Ok(Some(format!(
            "{},{}",
            latitude.unwrap(),
            longitude.unwrap()
        )))
    } else {
        Ok(None)
    }
}

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

fn bytes_to_geolocation(vec: &[u8]) -> Result<f64> {
    if vec.len() != 24 {
        return Err(anyhow!("Error: Vector must contain exactly 24 u8 values."));
    }

    let mut coord = [0u32; 6];
    for i in 0..6 {
        coord[i] = u32::from_le_bytes(vec[i * 4..(i + 1) * 4].try_into().unwrap());
    }

    let degrees = coord[0] as f64 / coord[1] as f64;
    let minutes = coord[2] as f64 / coord[3] as f64;
    let seconds = coord[4] as f64 / coord[5] as f64;

    Ok(degrees + minutes / 60.0 + seconds / 3600.0)
}

/// Converts a string to a byte vector in UCS-2 little-endian format.
fn string_to_ucs2_little_endian(input: &str) -> Vec<u8> {
    input
        .encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect()
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

    #[test]
    fn test_vec_u8_to_geoloction() {
        let coord: [u8; 24] = [
            0x2A, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x18, 0x00, 0x00, 0x00, 0x01, 0x00,
            0x00, 0x00, 0xE0, 0x07, 0x00, 0x00, 0x64, 0x00, 0x00, 0x00,
        ]; // replace with your coordinates
        let result = bytes_to_geolocation(&coord).unwrap();
        assert_eq!(result, 42.4056);
    }

    #[test]
    fn test_get_exif_location() -> Result<()> {
        /*
         0)  GPSLatitudeRef = N
         1)  GPSLatitude = 43 28 5.67599999 (43/1 28/1 567599999/100000000)
         2)  GPSLongitudeRef = E
         3)  GPSLongitude = 11 52 48.6179999 (11/1 52/1 486179999/10000000)
        */

        let source_file = PathBuf::from("testdata/gps/DSCN0029.jpg");

        let result = get_exif_location(&source_file).unwrap();
        assert_eq!(
            result,
            Some("43.468243333330555,11.880171666638889".to_string())
        );

        Ok(())
    }
}
