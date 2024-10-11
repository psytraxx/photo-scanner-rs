use anyhow::Result;
use std::path::Path;
use tracing::debug;
use xmp_toolkit::{xmp_ns::DC, IterOptions, OpenFileOptions, XmpFile, XmpMeta};

const XMP_DESCRIPTION: &str = "description";

fn new(path: &Path) -> Result<XmpFile> {
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

pub fn write_xmp_description(text: &str, path: &Path) -> Result<()> {
    let mut xmp_file = new(path)?;
    // Step 2: Try to extract existing XMP metadata
    let mut xmp = if let Some(existing_xmp) = xmp_file.xmp() {
        debug!("XMP metadata exists. Parsing it...");
        existing_xmp
    } else {
        debug!("No XMP metadata found. Creating a new one.");
        XmpMeta::new()?
    };

    xmp.set_localized_text(DC, XMP_DESCRIPTION, None, "x-default", text)?;

    xmp_file.put_xmp(&xmp)?;
    xmp_file.close();

    Ok(())
}

pub fn extract_persons(path: &Path) -> Result<Vec<String>> {
    let mut xmp_file = new(path)?;
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

        // Check that the description has been written correctly
        let faces = extract_persons(path)?;
        assert_eq!(faces.len(), 1);

        Ok(())
    }
}
