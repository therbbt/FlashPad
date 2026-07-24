// Shared by MarkdownEditor's Tiptap Link extension config, App.svelte's
// openLink, and UpdateDialog's release-notes renderer, so the actual set of
// protocols we're willing to hand to the OS opener never drifts between the
// three places that touch it. A bare email address (no "mailto:" prefix) is
// also allowed through, since that's the only realistic way a mailto: link
// ever gets created other than an explicit markdown link.
export const isAllowedLinkUrl = (url: string): boolean =>
  /^(https?:|mailto:)/i.test(url) || /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(url);
