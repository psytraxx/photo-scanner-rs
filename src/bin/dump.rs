use anyhow::{anyhow, Result};
use chrono::{DateTime, Datelike, FixedOffset, NaiveDateTime};
use mongodb::bson::{doc, Document};
use mongodb::options::{ClientOptions, Credential, ServerAddress};
use mongodb::{Client, Collection};
use photo_scanner::domain::{file_utils::list_jpeg_files, ports::XMPMetadata};
use photo_scanner::outbound::xmp::XMPToolkitMetadata;
use regex::Regex;
use std::path::Path;
use std::time::Duration;
use tracing::{error, info, warn};

/// Main entry
///  point.
#[tokio::main]
async fn main() -> Result<()> {
    // Set up tracing for logging.
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .without_time()
        .with_writer(std::io::stdout)
        .init();
    // Get the folder path from command line arguments.
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        //return Err(anyhow!("Please provide a path to the folder."));
    }

    let credential = Some(
        Credential::builder()
            .password("up2dateup2date".to_string())
            .username("symfony".to_string())
            .source(Some("admin".into()))
            .build(),
    );

    let server = ServerAddress::Tcp {
        host: "dot.dynamicflash.de".into(),
        port: Some(27017),
    };

    let client_options = ClientOptions::builder()
        .app_name(Some("Rust image classifier S3".into()))
        .connect_timeout(Some(Duration::from_secs(1)))
        .server_selection_timeout(Some(Duration::from_secs(1)))
        .default_database(Some("photos".to_string()))
        .direct_connection(Some(true))
        .credential(credential)
        .hosts(vec![server])
        .build();

    // Get a handle to the deployment.
    let client: Client = Client::with_options(client_options)?;
    let db = client.database("photos");
    let collection = db.collection::<Document>("exif_import1");

    //let root_path = PathBuf::from(&args[1]);

    let root_path = "/mnt/data/Photos/photos/";

    let files = list_jpeg_files(root_path)?;

    let xmp = XMPToolkitMetadata::new();

    for f in &files {
        let re = Regex::new(r"/photos/(\d{4})/").unwrap();
        let year: Option<i32> = re
            .captures(f.to_str().unwrap())
            .and_then(|caps| caps.get(1))
            .and_then(|m| m.as_str().parse::<i32>().ok());
        let year = year.unwrap();

        match xmp.get_created(f) {
            Ok(created) => {
                if created.year() != year {
                    warn!(
                        "Year mismatch: in metadata {} --> year folder {}, {}",
                        created.year(),
                        year,
                        f.display(),
                    );
                    match repair(&xmp, &collection, f, &year).await {
                        Ok(_) => info!("Restored after year missmatch: {}", f.display()),
                        Err(e) => {
                            error!("Error: {:?}", e);
                            continue;
                        }
                    }
                } else {
                    info!("OK {}: {:?}", f.display(), created)
                }
            }
            Err(e) => {
                warn!("Trying to restore {}: {:?}", f.display(), e);

                match repair(&xmp, &collection, f, &year).await {
                    Ok(_) => info!("Restored after missing metadata: {}", f.display()),
                    Err(e) => error!("Error: {:?}", e),
                }
            }
        }
    }

    Ok(())
}

async fn repair(
    xmp: &XMPToolkitMetadata,
    collection: &Collection<Document>,
    f: &Path,
    year: &i32,
) -> Result<()> {
    let file_name = f.file_name().unwrap().to_str().unwrap();

    let filter = doc! { "FileName": file_name,  "Directory": { "$regex": format!("{}", year) } };

    match collection.find_one(filter).await {
        Ok(Some(result)) => {
            let datetime_original = result
                .get("DateTimeOriginal")
                .and_then(|bson| bson.as_str());

            let create_date = result.get("CreateDate").and_then(|bson| bson.as_str());

            let modified_date = result.get("ModifyDate").and_then(|bson| bson.as_str());

            let json_created_new = datetime_original.or(create_date).or(modified_date);

            //
            if json_created_new.is_none() {
                return Err(anyhow!(
                    "Unable to find some date in existing mongodb entry: {:?}",
                    f.display()
                ));
            }

            let json_created_new = [datetime_original, create_date, modified_date]
                .iter()
                .find_map(|&date| {
                    if let Some(date_str) = date {
                        if let Ok(naive) =
                            NaiveDateTime::parse_from_str(date_str, "%Y:%m:%d %H:%M:%S")
                        {
                            let year_parsed = naive.year();
                            if year_parsed == *year {
                                return Some(date_str);
                            }
                        }
                    }
                    None
                });

            if json_created_new.is_none() {
                return Err(anyhow!(
                    "Unable to find some date for year : {:?} datetime_original{:?} create_date {:?} modified_date {:?}",
                    f.display(),
                    datetime_original,
                    create_date,
                    modified_date
                ));
            }

            let fixed_offset = FixedOffset::east_opt(0).unwrap(); // assume UTC
            match NaiveDateTime::parse_from_str(json_created_new.unwrap(), "%Y:%m:%d %H:%M:%S") {
                Ok(naive) => {
                    let datetime: DateTime<FixedOffset> =
                        DateTime::from_naive_utc_and_offset(naive, fixed_offset);
                    info!("Restoring date for {}: {:?}", f.display(), datetime);
                    xmp.set_created(f, &datetime)?;
                    Ok(())
                }
                Err(e) => Err(anyhow!("datetime error: {:?}", e)),
            }
        }
        Ok(None) => Err(anyhow!("No mongodb entry found for {}", file_name)),
        Err(e) => Err(anyhow!("Error: {:?}", e)),
    }
}
