const REPO = 'therbbt/FlashPad';

export interface ReleaseNotes {
  version: string;
  body: string;
  htmlUrl: string;
  publishedAt: string | null;
}

// Fetched directly from the GitHub REST API by tag, not from the updater
// plugin's latest.json manifest - that manifest only ever carries the
// single newest release's notes, which is the wrong thing to show once
// the user is already running a specific (possibly older-than-latest)
// version. A plain fetch works here: tauri.conf.json's CSP is disabled,
// and GitHub's public REST API sends permissive CORS headers.
export const fetchReleaseNotes = async (version: string): Promise<ReleaseNotes | null> => {
  const tag = version.startsWith('v') ? version : `v${version}`;
  const res = await fetch(`https://api.github.com/repos/${REPO}/releases/tags/${tag}`, {
    headers: { Accept: 'application/vnd.github+json' },
  });
  if (!res.ok) return null;
  const data = (await res.json()) as { body: string | null; html_url: string; published_at: string | null };
  return {
    version,
    body: data.body ?? '',
    htmlUrl: data.html_url,
    publishedAt: data.published_at,
  };
};
