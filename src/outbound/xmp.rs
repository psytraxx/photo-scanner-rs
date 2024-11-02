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
    fn get_xmp_description(&self, path: &Path) -> Result<Option<String>> {
        let mut xmp_file = open(path)?;
        let xmp = xmp_file.xmp().context("No XMP metadata found")?;
        xmp_file.close();

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

    fn get_xmp_geolocation(&self, path: &Path) -> Result<Option<String>> {
        let mut xmp_file = open(path)?;
        let xmp = xmp_file.xmp().context("No XMP metadata found")?;
        xmp_file.close();

        let longitude = xmp.property(EXIF, "GPSLongitude").map(|val| val.value);
        let latitude = xmp.property(EXIF, "GPSLatitude").map(|val| val.value);

        if let (Some(latitude), Some(longitude)) = (latitude, longitude) {
            if let (Some(latitude), Some(longitude)) = (dms_to_dd(&latitude), dms_to_dd(&longitude))
            {
                Ok(Some(format!("{},{}", latitude, longitude)))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    fn write_xmp_description(&self, text: &str, path: &Path) -> Result<()> {
        let mut xmp_file = open(path)?;

        let mut xmp = match xmp_file.xmp() {
            Some(existing_xmp) => {
                debug!("XMP metadata exists. Parsing it...");
                existing_xmp
            }
            None => {
                debug!("No XMP metadata found. Creating a new one.");
                XmpMeta::new()?
            }
        };

        xmp.set_localized_text(DC, XMP_DESCRIPTION, None, "x-default", text)?;

        xmp_file.put_xmp(&xmp)?;
        xmp_file.close();

        Ok(())
    }

    fn extract_persons(&self, path: &Path) -> Result<Vec<String>> {
        let mut xmp_file = open(path)?;
        let result = match xmp_file.xmp() {
            Some(xmp) => {
                let names: Vec<String> = xmp
                    .iter(
                        IterOptions::default()
                            .schema_ns("http://www.metadataworkinggroup.com/schemas/regions/"),
                    )
                    .filter(|x| x.name.ends_with("mwg-rs:Name"))
                    .map(|x| x.value.value)
                    .collect();
                debug!("Names in XMP data: {:?}", names);
                names
            }
            None => {
                debug!("No XMP metadata found.");
                Vec::new()
            }
        };
        xmp_file.close();
        Ok(result)
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
    // Remove the directional character (N/S) and split by comma
    let (coords, direction) = dms.split_at(dms.len() - 1);
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
    use std::path::Path;

    #[test]
    fn test_extract_persons() -> Result<()> {
        tracing_subscriber::fmt()
            .with_max_level(Level::DEBUG)
            .with_ansi(true)
            .with_target(false)
            .without_time()
            .init();

        let path = Path::new("testdata/picasa/PXL_20230408_060152625.jpg");

        let tool = XMPToolkitMetadata::new();

        // Check that the description has been written correctly
        let faces = tool.extract_persons(path)?;
        assert_eq!(faces.len(), 1);

        Ok(())
    }

    #[test]
    fn test_get_xmp_description() -> Result<()> {
        let path = Path::new("testdata/picasa/PXL_20230408_060152625.jpg");
        let tool = XMPToolkitMetadata::new();

        // Check that the description has been written correctly
        let description = tool.get_xmp_description(path)?;
        assert!(description.is_some());

        Ok(())
    }

    #[test]
    fn test_get_xmp_geolocation() -> Result<()> {
        let path = Path::new("testdata/gps/DSCN0029.jpg");
        let tool = XMPToolkitMetadata::new();

        // Check that the description has been written correctly
        let description = tool.get_xmp_geolocation(path)?;
        assert!(description.is_some());

        assert_eq!(
            "43.468243333333334,11.880171666666667",
            description.unwrap()
        );

        Ok(())
    }

    #[test]
    fn test_get_xmp_description_missing() -> Result<()> {
        let path = Path::new("testdata/sizilien/4L2A3805.jpg");
        let tool = XMPToolkitMetadata::new();
        // Check that the description has been written correctly
        let description = tool.get_xmp_description(path)?;
        assert!(description.is_none());

        Ok(())
    }
}
