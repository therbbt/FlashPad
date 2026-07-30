import { writable } from 'svelte/store';

// Current vim mode label for the footer badge ('NORMAL' | 'INSERT' |
// 'VISUAL' | 'V-LINE' | 'V-BLOCK' | 'REPLACE'), or null when vim mode is
// off or a Markdown note is open - PlainTextEditor is the only writer.
export const vimModeIndicator = writable<string | null>(null);
