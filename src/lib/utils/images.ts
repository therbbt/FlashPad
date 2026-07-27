// Shared by MarkdownEditor's paste/drop image handling - the only formats a
// webview can render natively via <img> without any conversion, and the
// only ones worth base64-embedding directly into a note's content.
export const ALLOWED_IMAGE_MIME_TYPES = ['image/png', 'image/jpeg', 'image/gif', 'image/webp'];

export const isAllowedImageMimeType = (type: string): boolean => ALLOWED_IMAGE_MIME_TYPES.includes(type);

// Native OS drag-and-drop (see MarkdownEditor.svelte's onDragDropEvent
// handler) only gives us a file path, not a MIME type, so extension is all
// we have to pre-filter on before asking the Rust side to read the file.
const ALLOWED_IMAGE_EXTENSIONS = ['png', 'jpg', 'jpeg', 'gif', 'webp'];

export const isAllowedImagePath = (path: string): boolean => {
  const extension = path.split('.').pop()?.toLowerCase();
  return Boolean(extension && ALLOWED_IMAGE_EXTENSIONS.includes(extension));
};
