use std::path::Path;

use crate::domain::ports::FileMeta;
use anyhow::Result;
use async_trait::async_trait;
use tracing::debug;
use xmp_toolkit::{OpenFileOptions, XmpFile, XmpMeta, XmpValue};

#[derive(Debug, Clone)]
pub struct XMP {}

#[async_trait]
impl FileMeta for XMP {
    async fn write(&self, text: &str, path: &Path) -> Result<()> {
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

        // Step 2: Try to extract existing XMP metadata
        let mut xmp = if let Some(existing_xmp) = xmp_file.xmp() {
            debug!("XMP metadata exists. Parsing it...");
            existing_xmp
        } else {
            debug!("No XMP metadata found. Creating a new one.");
            XmpMeta::new()?
        };

        /*  xmp.iter(IterOptions::default()).for_each(|p| {
            info!("{:?}", p);
        });*/

        let new_value: XmpValue<String> = XmpValue::new(text.into());
        xmp.set_property(xmp_toolkit::xmp_ns::DC, "description", &new_value)?;

        xmp_file.put_xmp(&xmp)?;
        xmp_file.close();

        Ok(())
    }
}
