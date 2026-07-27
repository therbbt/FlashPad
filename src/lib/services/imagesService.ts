import { invoke } from '@tauri-apps/api/core';

// Reads a file dropped onto the window (see MarkdownEditor.svelte's
// onDragDropEvent handler) and returns it as a base64 data: URI.
export const readDroppedImage = (path: string): Promise<string> => invoke('read_dropped_image', { path });
