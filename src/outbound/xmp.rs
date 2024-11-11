use crate::domain::ports::XMPMetadata;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use std::path::Path;
use tracing::{debug, warn};
use xmp_toolkit::{
    xmp_gps::{exif_latitude_to_decimal, exif_longitude_to_decimal},
    xmp_ns::{DC, EXIF, PHOTOSHOP, XMP},
    IterOptions, OpenFileOptions, XmpDateTime, XmpFile, XmpMeta, XmpTime, XmpTimeZone, XmpValue,
};

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
        let mut xmp_file = open(path, false)?;
        let xmp = xmp_file
            .xmp()
            .context("XMPMetadata not found get_description")?;

        match xmp.localized_text(DC, "description", None, "x-default") {
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
        let mut xmp_file = open(path, false)?;
        let xmp = xmp_file
            .xmp()
            .context("XMPMetadata not found get_geolocation")?;

        // Fetch the GPS coordinates
        let gps_longitude = xmp.property(EXIF, "GPSLongitude").map(|val| val.value);
        let gps_latitude = xmp.property(EXIF, "GPSLatitude").map(|val| val.value);

        // If both coordinates are present
        if let (Some(latitude), Some(longitude)) = (gps_latitude, gps_longitude) {
            // Convert them to decimal degrees
            if let (Some(latitude), Some(longitude)) = (
                exif_latitude_to_decimal(&latitude),
                exif_longitude_to_decimal(&longitude),
            ) {
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

    fn set_description(&self, path: &Path, text: &str) -> Result<()> {
        let mut xmp_file = open(path, true)?;
        let mut xmp = xmp_file
            .xmp()
            .context("XMPMetadata not found set_description")
            .or(XmpMeta::new())?;

        xmp.set_localized_text(DC, "description", None, "x-default", text)?;

        xmp_file.put_xmp(&xmp)?;

        // this writes the XMP data to the file
        xmp_file.close();

        Ok(())
    }

    fn get_persons(&self, path: &Path) -> Result<Vec<String>> {
        let mut xmp_file = open(path, false)?;
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

    fn get_created(&self, path: &Path) -> Result<DateTime<FixedOffset>> {
        let mut xmp_file = open(path, false)?;
        let xmp = xmp_file
            .xmp()
            .context("XMPMetadata not found get_created")?;

        let created = xmp
            .property_date(XMP, "CreateDate")
            .or_else(|| xmp.property_date(EXIF, "DateTimeOriginal"))
            .or_else(|| xmp.property_date(PHOTOSHOP, "DateCreated"))
            .ok_or(anyhow!(
                "Neither xmp:CreateDate, exif:DateTimeOriginal nor photoshop:DateCreated property found"
            ))?;

        let mut created = created.value;

        debug!("Created in XMP data: {:?}", created);

        // Set timezone to UTC+1 (Zurich) if not present
        if let Some(ref mut time) = created.time {
            if time.time_zone.is_none() {
                time.time_zone = Some(XmpTimeZone { hour: 1, minute: 0 }); // assume UTC
            }
        } else {
            created.time = Some(XmpTime {
                hour: 0,
                minute: 0,
                second: 0,
                nanosecond: 0,
                time_zone: Some(XmpTimeZone { hour: 1, minute: 0 }), // assume UTC
            });
        }

        let created: DateTime<FixedOffset> = created.try_into()?;
        Ok(created)
    }

    fn set_created(&self, path: &Path, created: &DateTime<FixedOffset>) -> Result<()> {
        let mut xmp_file = open(path, true)?;
        let mut xmp = xmp_file
            .xmp()
            .context("XMPMetadata not found set_created")
            .or(XmpMeta::new())?;

        let created: XmpDateTime = created.into();
        let created = XmpValue::new(created);
        xmp.set_property_date(XMP, "CreateDate", &created)?;
        xmp.set_property_date(PHOTOSHOP, "DateCreated", &created)?;
        xmp.set_property_date(EXIF, "DateTimeOriginal", &created)?;

        xmp_file.put_xmp(&xmp)?;

        // this writes the XMP data to the file
        xmp_file.close();

        Ok(())
    }
}

fn open(path: &Path, allow_update: bool) -> Result<XmpFile> {
    let mut xmp_file = XmpFile::new()?;

    fn get_options(allow_update: bool) -> OpenFileOptions {
        if allow_update {
            OpenFileOptions::default().only_xmp().for_update()
        } else {
            OpenFileOptions::default().only_xmp().for_read()
        }
    }

    // Try opening the file with the smart handler
    if xmp_file
        .open_file(path, get_options(allow_update).use_smart_handler())
        .is_err()
    {
        warn!(
            "No smart handler available for file {:?}. Trying packet scanning.",
            path
        );
        xmp_file.open_file(path, get_options(allow_update).use_packet_scanning())?;
    }

    // Return the XmpFile instance
    Ok(xmp_file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, TimeZone, Utc};
    use std::sync::Once;
    use std::{
        fs::{copy, remove_file},
        path::{Path, PathBuf},
    };
    use tracing::Level;

    static INIT: Once = Once::new();

    pub fn initialize() {
        INIT.call_once(|| {
            tracing_subscriber::fmt()
                .with_max_level(Level::DEBUG)
                .with_ansi(true)
                .with_target(false)
                .without_time()
                .init();
        });
    }

    #[test]
    fn test_get_persons() -> Result<()> {
        initialize();
        let path = Path::new("testdata/example-persons.jpg");

        let tool = XMPToolkitMetadata::new();

        let faces = tool.get_persons(path)?;
        assert_eq!(faces.len(), 1);

        Ok(())
    }

    /// Test that that we are able to write and read the created date from the XMP metadata
    #[test]
    fn test_set_and_get_created() -> Result<()> {
        initialize();
        let temp_dir = tempfile::tempdir()?;
        let destination_file_path = temp_dir.path().join("4L2A3805.jpg");

        // Copy an existing JPEG file to the temporary directory
        let source_file = PathBuf::from("testdata/example-full.jpg");
        copy(&source_file, &destination_file_path)?;

        let tool = XMPToolkitMetadata::new();

        let created_in = NaiveDate::from_ymd_opt(1999, 1, 1).unwrap();
        let created_in = created_in.and_hms_opt(10, 0, 0).unwrap();
        let created_in = Utc
            .from_utc_datetime(&created_in)
            .with_timezone(&FixedOffset::east_opt(3600).unwrap());
        tool.set_created(&destination_file_path, &created_in)?;

        let created_out = tool.get_created(&destination_file_path)?;
        assert_eq!(created_in, created_out);

        // Clean up by deleting the temporary file
        remove_file(&destination_file_path)?;

        Ok(())
    }

    /// Test that the get_created function returns the correct date when the file has no XMP metadata but EXIF metadata
    #[test]
    fn test_get_created_no_xmp() -> Result<()> {
        initialize();
        let source_file = PathBuf::from("testdata/example-no-xmp.jpg");

        let tool = XMPToolkitMetadata::new();
        // Check that the description has been written correctly
        let created_out = tool.get_created(&source_file)?;

        let created_stored = NaiveDate::from_ymd_opt(2023, 10, 9).unwrap();
        let created_stored = created_stored.and_hms_opt(10, 33, 31).unwrap();
        let created_stored = Utc
            .from_utc_datetime(&created_stored)
            .with_timezone(&FixedOffset::east_opt(3600).unwrap());
        assert_eq!(created_stored, created_out);

        Ok(())
    }

    /// Test that the get_created function returns the correct date when the file has no XMP metadata and no EXIF metadata but still Photoshop metadata
    #[test]
    fn test_get_created_no_xmp_no_exif() -> Result<()> {
        initialize();
        let source_file = PathBuf::from("testdata/example-no-xmp-no-exif.jpg");

        let tool = XMPToolkitMetadata::new();

        let created_out = tool.get_created(&source_file)?;

        let created_stored = NaiveDate::from_ymd_opt(2023, 10, 9).unwrap();
        let created_stored = created_stored.and_hms_opt(10, 33, 31).unwrap();
        let created_stored = Utc
            .from_utc_datetime(&created_stored)
            .with_timezone(&FixedOffset::east_opt(3600).unwrap());
        assert_eq!(created_stored, created_out);

        Ok(())
    }

    /// Test that the get_created function returns an error when the file has no XMP, EXIF or Photoshop metadata
    #[test]
    fn test_get_created_no_xmp_no_exif_no_photoshop() -> Result<()> {
        initialize();
        let source_file = PathBuf::from("testdata/example-no-xmp-no-exif-no-photoshop.jpg");

        let tool = XMPToolkitMetadata::new();

        let created_out = tool.get_created(&source_file);

        assert!(created_out.is_err());

        Ok(())
    }

    #[test]
    fn test_set_and_get_description() -> Result<()> {
        initialize();
        let temp_dir = tempfile::tempdir()?;
        let destination_file_path = temp_dir.path().join("example-full.jpg");

        // Copy an existing JPEG file to the temporary directory
        let source_file = PathBuf::from("testdata/example-full.jpg");
        copy(&source_file, &destination_file_path)?;

        let tool = XMPToolkitMetadata::new();

        let test_description = "This is a test description";
        tool.set_description(&destination_file_path, test_description)?;

        // Check that the description has been written correctly
        let description = tool.get_description(&destination_file_path)?;
        assert_eq!(description, Some(test_description.to_string()));

        // Clean up by deleting the temporary file
        remove_file(&destination_file_path)?;

        Ok(())
    }

    #[test]
    fn test_set_and_get_description_no_existing_xmp() -> Result<()> {
        initialize();
        let temp_dir = tempfile::tempdir()?;
        let destination_file_path = temp_dir.path().join("example-no-xmp.jpg");

        // Copy an existing JPEG file to the temporary directory
        let source_file = PathBuf::from("testdata/example-no-xmp.jpg");
        copy(&source_file, &destination_file_path)?;

        let tool = XMPToolkitMetadata::new();

        let test_description = "This is a test description";
        tool.set_description(&destination_file_path, test_description)?;

        // Check that the description has been written correctly
        let description = tool.get_description(&destination_file_path)?;
        assert_eq!(description, Some(test_description.to_string()));

        // Clean up by deleting the temporary file
        remove_file(&destination_file_path)?;

        Ok(())
    }

    #[test]
    fn test_get_geolocation() -> Result<()> {
        initialize();
        let path = Path::new("testdata/example-gps.jpg");
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
        initialize();
        let path = Path::new("testdata/example-full.jpg");
        let tool = XMPToolkitMetadata::new();
        // Check that the description has been written correctly
        let description = tool.get_description(path)?;
        assert!(description.is_none());

        Ok(())
    }
}
