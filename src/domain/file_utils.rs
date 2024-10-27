use std::fs::read_dir;
use std::path::{Path, PathBuf};

/// Function to list files in a directory and its subdirectories.
pub fn list_jpeg_files<P: AsRef<Path>>(path: P) -> anyhow::Result<Vec<PathBuf>> {
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
