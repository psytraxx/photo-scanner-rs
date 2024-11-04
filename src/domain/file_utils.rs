use anyhow::Result;
use std::fs::read_dir;
use std::path::{Path, PathBuf};

/// Function to list files in a directory and its subdirectories.
pub fn list_jpeg_files<P: AsRef<Path>>(path: P) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in read_dir(path)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            // Recursively traverse subdirectories
            files.extend(list_jpeg_files(path)?);
        } else if is_jpeg(&path) {
            // Only include JPEG files
            files.push(path);
        }
    }
    Ok(files)
}

/// Function to check if the path has a valid JPEG extension.
fn is_jpeg(path: &Path) -> bool {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => matches!(ext.to_ascii_lowercase().as_str(), "jpg" | "jpeg"),
        None => false, // No extension present
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{create_dir, File};
    use tempfile::tempdir;

    #[test]
    fn test_list_jpeg_files() {
        let tmp_dir = tempdir().unwrap();

        // Create files with different extensions
        File::create(tmp_dir.path().join("image1.JPG")).unwrap();
        File::create(tmp_dir.path().join("image2.jpeg")).unwrap();
        File::create(tmp_dir.path().join("image3.png")).unwrap();

        // Create subdirectory and add a JPEG file
        let sub_dir = tmp_dir.path().join("subdir");
        create_dir(&sub_dir).unwrap();
        File::create(sub_dir.join("image4.jpg")).unwrap();

        // Get list of JPEG files
        let jpeg_files = list_jpeg_files(tmp_dir.path()).unwrap();

        // Assert that only the JPEG files are listed
        assert_eq!(jpeg_files.len(), 3);
        assert!(jpeg_files.contains(&tmp_dir.path().join("image1.JPG")));
        assert!(jpeg_files.contains(&tmp_dir.path().join("image2.jpeg")));
        assert!(jpeg_files.contains(&sub_dir.join("image4.jpg")));
    }

    #[test]
    fn test_is_jpeg() {
        assert!(is_jpeg(Path::new("image.jpg")));
        assert!(is_jpeg(Path::new("image.jpeg")));
        assert!(!is_jpeg(Path::new("image.png")));
        assert!(!is_jpeg(Path::new("image")));
    }
}
