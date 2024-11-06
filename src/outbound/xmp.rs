use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::Path;
use tracing::debug;
use xmp_toolkit::{
    xmp_ns::{DC, EXIF},
    IterOptions, OpenFileOptions, XmpFile, XmpMeta,
};

use crate::domain::ports::XMPMetadata;

const XMP_DESCRIPTION: &str = "description";

#[derive(Debug, Clone, Default)]
pub struct XMPToolkitMetadata;

impl XMPToolkitMetadata {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl XMPMetadata for XMPToolkitMetadata {
    fn get_description(&self, path: &Path) -> Result<Option<String>> {
        let mut xmp_file = open(path)?;
        let xmp = xmp_file
            .xmp()
            .context("XMPMetadata not found get_description")?;

        match xmp.localized_text(DC, XMP_DESCRIPTION, None, "x-default") {
            Some(description) => {
                let description = description.0.value;
                debug!("Description in XMP data: {:?}", description);
                Ok(Some(description))
            }
            None => {
                debug!("No description in XMP data.");
                Ok(None)
            }
        }
    }

    fn get_geolocation(&self, path: &Path) -> Result<Option<String>> {
        let mut xmp_file = open(path)?;
        let xmp = xmp_file
            .xmp()
            .context("XMPMetadata not found get_geolocation")?;

        // Fetch the GPS coordinates
        let gps_longitude = xmp.property(EXIF, "GPSLongitude").map(|val| val.value);
        let gps_latitude = xmp.property(EXIF, "GPSLatitude").map(|val| val.value);

        // If both coordinates are present
        if let (Some(latitude), Some(longitude)) = (gps_latitude, gps_longitude) {
            // Convert them to decimal degrees
            if let (Some(latitude), Some(longitude)) = (dms_to_dd(&latitude), dms_to_dd(&longitude))
            {
                // Format the coordinates and return them
                Ok(Some(format!("{},{}", latitude, longitude)))
            } else {
                // If the conversion fails, return None
                debug!("Failed to convert GPS coordinates to decimal degrees.");
                Ok(None)
            }
        } else {
            // If either coordinate is missing, return None
            debug!("Missing GPS coordinates in XMP data.");
            Ok(None)
        }
    }

    fn set_description(&self, text: &str, path: &Path) -> Result<()> {
        let mut xmp_file = open(path)?;
        let mut xmp = xmp_file
            .xmp()
            .context("XMPMetadata not found get_persons")
            .or(XmpMeta::new())?;

        xmp.set_localized_text(DC, XMP_DESCRIPTION, None, "x-default", text)?;

        xmp_file.put_xmp(&xmp)?;

        // this writes the XMP data to the file
        xmp_file.close();

        Ok(())
    }

    fn get_persons(&self, path: &Path) -> Result<Vec<String>> {
        let mut xmp_file = open(path)?;
        let xmp = xmp_file
            .xmp()
            .context("XMPMetadata not found get_persons")?;

        let names: Vec<String> = xmp
            .iter(
                IterOptions::default()
                    .schema_ns("http://www.metadataworkinggroup.com/schemas/regions/"),
            )
            .filter(|x| x.name.ends_with("mwg-rs:Name"))
            .map(|x| x.value.value)
            .collect();
        debug!("Names in XMP data: {:?}", names);

        Ok(names)
    }
}
fn open(path: &Path) -> Result<XmpFile> {
    // Step 1: Open the JPEG file with XmpFile for reading and writing XMP metadata
    let mut xmp_file = XmpFile::new()?;

    xmp_file
        .open_file(
            path,
            OpenFileOptions::default()
                .only_xmp()
                .for_update()
                .use_smart_handler(),
        )
        .or_else(|_| {
            xmp_file.open_file(
                path,
                OpenFileOptions::default()
                    .only_xmp()
                    .for_update()
                    .use_packet_scanning(),
            )
        })?;

    Ok(xmp_file)
}

/// Convert DMS (degrees, minutes, seconds) to decimal degrees
fn dms_to_dd(dms: &str) -> Option<f64> {
    // Remove the directional character (N/S/E/W) and split by comma
    let (coords, direction) = dms.split_at(dms.len().saturating_sub(1));
    let parts: Vec<&str> = coords.split(',').collect();

    if parts.len() != 2 {
        return None;
    }

    let degrees = parts[0].trim().parse::<f64>().ok()?;
    let minutes = parts[1].trim().parse::<f64>().ok()?;

    // Convert DMS to decimal degrees
    let dd = degrees + (minutes / 60.0);

    // Adjust for direction
    match direction.trim() {
        "N" | "E" => Some(dd),  // North and East are positive
        "S" | "W" => Some(-dd), // South and West are negative
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use tracing::Level;

    use super::*;
    use std::{
        fs::{copy, remove_file},
        path::{Path, PathBuf},
    };

    #[test]
    fn test_get_persons() -> Result<()> {
        tracing_subscriber::fmt()
            .with_max_level(Level::DEBUG)
            .with_ansi(true)
            .with_target(false)
            .without_time()
            .init();

        let path = Path::new("testdata/picasa/PXL_20230408_060152625.jpg");

        let tool = XMPToolkitMetadata::new();

        // Check that the description has been written correctly
        let faces = tool.get_persons(path)?;
        assert_eq!(faces.len(), 1);

        Ok(())
    }

    #[test]
    fn test_set_and_get_description() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let destination_file_path = temp_dir.path().join("4L2A3805.jpg");

        // Copy an existing JPEG file to the temporary directory
        let source_file = PathBuf::from("testdata/sizilien/4L2A3805.jpg");
        copy(&source_file, &destination_file_path)?;

        let tool = XMPToolkitMetadata::new();

        let test_description = "This is a test description";
        tool.set_description(test_description, &destination_file_path)?;

        // Check that the description has been written correctly
        let description = tool.get_description(&destination_file_path)?;
        assert_eq!(description, Some(test_description.to_string()));

        // Clean up by deleting the temporary file
        remove_file(&destination_file_path)?;

        Ok(())
    }

    #[test]
    fn test_set_and_get_description_no_existing_xmp() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let destination_file_path = temp_dir.path().join("4L2A3805.jpg");

        // Copy an existing JPEG file to the temporary directory
        let source_file = PathBuf::from("testdata/sizilien/4L2A3805-no-xmp.jpg");
        copy(&source_file, &destination_file_path)?;

        let tool = XMPToolkitMetadata::new();

        let test_description = "This is a test description";
        tool.set_description(test_description, &destination_file_path)?;

        // Check that the description has been written correctly
        let description = tool.get_description(&destination_file_path)?;
        assert_eq!(description, Some(test_description.to_string()));

        // Clean up by deleting the temporary file
        remove_file(&destination_file_path)?;

        Ok(())
    }

    #[test]
    fn test_get_geolocation() -> Result<()> {
        let path = Path::new("testdata/gps/DSCN0029.jpg");
        let tool = XMPToolkitMetadata::new();

        // Check that the description has been written correctly
        let description = tool.get_geolocation(path)?;
        assert!(description.is_some());

        assert_eq!(
            "43.468243333333334,11.880171666666667",
            description.unwrap()
        );

        Ok(())
    }

    #[test]
    fn test_get_description_missing() -> Result<()> {
        let path = Path::new("testdata/sizilien/4L2A3805.jpg");
        let tool = XMPToolkitMetadata::new();
        // Check that the description has been written correctly
        let description = tool.get_description(path)?;
        assert!(description.is_none());

        Ok(())
    }

    #[test]
    fn test_dms_to_dd() {
        // Testing conversion of North and East coordinates
        assert_eq!(dms_to_dd("43,28.09460000N"), Some(43.468243333333334));
        assert_eq!(dms_to_dd("11,52.8103000E"), Some(11.880171666666667));

        // Testing conversion of South and West coordinates
        assert_eq!(dms_to_dd("43,28.09460000S"), Some(-43.468243333333334));
        assert_eq!(dms_to_dd("11,52.8103000W"), Some(-11.880171666666667));

        // Testing invalid inputs
        assert_eq!(dms_to_dd("43,28.09460000X"), None);
        assert_eq!(dms_to_dd("43.28.09460000X"), None);
        assert_eq!(dms_to_dd("40"), None);
    }
}
