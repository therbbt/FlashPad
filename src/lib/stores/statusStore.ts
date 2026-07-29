import { writable } from 'svelte/store';

// The footer status line - genuinely cross-cutting (set from notes CRUD,
// editor autosave, link-opening, database switching, window-hide), so it
// lives in its own tiny store rather than inside any one domain module.
export const status = writable('Ready');
