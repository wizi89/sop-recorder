use std::fs;
use std::path::Path;

/// Save the server-returned markdown as guide.md in the output directory.
pub fn save_markdown(output_dir: &Path, markdown: &str) -> Result<(), std::io::Error> {
    let path = output_dir.join("guide.md");
    fs::write(&path, markdown)?;
    log::info!("Markdown saved: {}", path.display());
    Ok(())
}
