use base64::{engine::general_purpose::STANDARD, Engine as _};
use std::path::Path;

// Mirrors ALLOWED_IMAGE_MIME_TYPES in src/lib/utils/images.ts. Native OS
// drag-and-drop only gives us a file path (no MIME type), so the extension
// is all we have to validate against on this side.
const MAX_FILE_SIZE_BYTES: u64 = 25 * 1024 * 1024;

fn mime_for_extension(extension: &str) -> Option<&'static str> {
    match extension.to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

// Reads a file dropped onto the window (via the native drag-drop event,
// which only ever gives us a path - see MarkdownEditor.svelte) and returns
// it as a base64 data: URI, matching how clipboard-pasted images are
// embedded directly into the note's content.
#[tauri::command]
pub fn read_dropped_image(path: String) -> Result<String, String> {
    let file_path = Path::new(&path);
    let mime = file_path
        .extension()
        .and_then(|ext| ext.to_str())
        .and_then(mime_for_extension)
        .ok_or_else(|| "Unsupported image type".to_string())?;

    let metadata = std::fs::metadata(file_path).map_err(|err| err.to_string())?;
    if metadata.len() > MAX_FILE_SIZE_BYTES {
        return Err("Image file is too large to embed".to_string());
    }

    let bytes = std::fs::read(file_path).map_err(|err| err.to_string())?;
    let encoded = STANDARD.encode(bytes);
    Ok(format!("data:{mime};base64,{encoded}"))
}
