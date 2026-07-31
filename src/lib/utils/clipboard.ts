// Line-ending translation between the editors' document model and the
// Windows clipboard convention.
//
// Both editors model a document as LF-only text (CodeMirror splits on
// /\r\n?|\n/ and stores lines without the separator; ProseMirror likewise),
// while Windows apps put - and expect - CRLF on the clipboard. Neither
// direction converts for us: Blink only rewrites LF to CRLF on its own
// internal copy path, not for text an editor writes itself via
// clipboardData.setData(), which is exactly what CodeMirror and ProseMirror
// both do. So text copied out of FlashPad arrives at a Win32 edit control as
// one long line, and CRLF text pasted in keeps its stray \r characters.

export const toLfNewlines = (text: string): string => text.replace(/\r\n?/g, '\n');

export const toCrlfNewlines = (text: string): string => text.replace(/\r\n?|\n/g, '\r\n');

// Only Windows wants CRLF - on macOS/Linux the same text would carry visible
// ^M into terminals and other plain-text targets.
export const prefersCrlfClipboard = (): boolean =>
  typeof navigator !== 'undefined' && /Win/i.test(navigator.userAgent);
