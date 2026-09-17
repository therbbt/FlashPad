import { StreamLanguage, LanguageSupport, type StreamParser } from '@codemirror/language';
import type { LanguageId } from '../utils/languageDetect';

// Hand-rolled CodeMirror "simple mode" for web server logs (nginx/Apache
// access + nginx error log lines) - same StreamLanguage approach as
// networkConfigLanguage.ts, for the same reason: there's no real grammar to
// build a Lezer parser from, just a handful of visually-useful token shapes
// (IPs, hostnames, bracketed timestamps/severity, HTTP methods) to pick out
// of otherwise-unstructured text.
//
// Deliberately doesn't try to atomically parse quoted strings (the request
// line, user-agent, referrer) the way networkConfigLanguage.ts does for
// vendor CLI strings - here that would hide exactly the parts worth
// highlighting (the method, the embedded referrer URL's own host). Quote
// characters are just left untagged, and whatever's between them still goes
// through the normal token rules below.

// A curated allowlist, not a real TLD registry - just enough to tell a real
// hostname ("insight.fiberdata.lan") apart from a dotted filename
// ("kumbro-logo.png") or a dotted version number ("139.0.0.0" in a
// User-Agent string), which are structurally identical otherwise. Includes
// common internal/lab pseudo-TLDs (lan, local, internal, corp, home, test)
// since these logs are as likely to be from a private network as the public
// internet.
const KNOWN_TLDS = new Set([
  'com', 'net', 'org', 'io', 'dev', 'app', 'co', 'info', 'biz', 'edu', 'gov', 'mil', 'int', 'name', 'pro',
  'local', 'lan', 'internal', 'corp', 'home', 'test', 'localdomain',
  'se', 'no', 'dk', 'fi', 'de', 'fr', 'uk', 'us', 'nl', 'be', 'ch', 'at', 'es', 'it', 'pl', 'ru', 'cn', 'jp',
  'kr', 'in', 'au', 'ca', 'br', 'mx', 'ai', 'me', 'tv', 'cc', 'ly',
]);

const HTTP_METHODS = new Set(['GET', 'POST', 'PUT', 'DELETE', 'HEAD', 'OPTIONS', 'PATCH', 'CONNECT', 'TRACE']);

const ERROR_LEVELS = new Set(['error', 'crit', 'alert', 'emerg', 'fatal']);
const LOW_PRIORITY_LEVELS = new Set(['info', 'notice', 'debug']);

// Colors an HTTP status code by class - 1xx/2xx (success) green, 3xx
// (redirect - not actually an error, but not a plain success either) amber,
// 4xx/5xx (client/server error) red. Only ever consulted right after an
// "HTTP/x.x" token (see expectStatus below), not for any bare 3-digit
// number - a byte count like the 517 in this exact log format can
// coincidentally be 3 digits too, and isn't a status code.
function statusTag(word: string): string {
  if (word[0] === '4' || word[0] === '5') return 'invalid';
  if (word[0] === '3') return 'atom';
  return 'string';
}

interface LogState {
  // Set right after matching a "scheme://" prefix so the very next token is
  // known to be a host (IP or domain), not just a bare word that happens to
  // look like one - this is what lets a raw-IP host in a URL (e.g.
  // "http://172.18.0.4:5000/...") get the IP color without needing the
  // TLD allowlist at all.
  afterScheme: boolean;
  // Set right after matching "HTTP/x.x" (the end of a request line) - the
  // closing quote and the single space that follow don't touch this, so it
  // survives until the very next word token, which in a combined-log-format
  // line is always the status code.
  expectStatus: boolean;
}

function isDottedIPv4(word: string): boolean {
  return /^\d{1,3}(\.\d{1,3}){3}$/.test(word);
}

function domainTld(word: string): string | null {
  const labels = word.split('.');
  if (labels.length < 2) return null;
  return labels[labels.length - 1].toLowerCase();
}

const parser: StreamParser<LogState> = {
  startState: () => ({ afterScheme: false, expectStatus: false }),
  token(stream, state) {
    if (stream.sol()) {
      state.afterScheme = false;
      state.expectStatus = false;
    }
    if (stream.eatSpace()) return null;

    if (state.afterScheme) {
      state.afterScheme = false;
      if (stream.match(/^\d{1,3}(\.\d{1,3}){3}(:\d+)?/)) return 'number';
      if (stream.match(/^[A-Za-z0-9-]+(\.[A-Za-z0-9-]+)*(:\d+)?/)) return 'propertyName';
    }

    if (stream.match(/^https?:\/\//i)) {
      state.afterScheme = true;
      return null;
    }

    // End of the request line ("GET /path HTTP/2.0") - the status code
    // immediately follows, past the closing quote and one space.
    if (stream.match(/^HTTP\/\d(?:\.\d)?/i)) {
      state.expectStatus = true;
      return null;
    }

    // Bracketed group: either a date/time stamp (dimmed, like a comment -
    // useful context but not what the eye should land on first) or a
    // severity level (color-coded by how bad it is).
    if (stream.peek() === '[') {
      const closeIndex = stream.string.indexOf(']', stream.pos);
      if (closeIndex !== -1) {
        const inner = stream.string.slice(stream.pos + 1, closeIndex);
        stream.pos = closeIndex + 1;
        const lower = inner.toLowerCase();
        if (ERROR_LEVELS.has(lower)) return 'invalid';
        if (lower === 'warn' || lower === 'warning') return 'keyword';
        if (LOW_PRIORITY_LEVELS.has(lower)) return 'meta';
        // Apache/nginx access-log timestamp shape ("09/Sep/2026:11:36:33
        // +0000") - checked as its own explicit pattern rather than a loose
        // "mostly digits/punctuation" one, since the month abbreviation's
        // letters would otherwise fail a purely-numeric check.
        if (/^\d{1,2}\/[A-Za-z]{3}\/\d{4}:\d{2}:\d{2}:\d{2} [+-]\d{4}$/.test(inner)) return 'meta';
        if (/^[\d/:. +apm-]+$/i.test(inner)) return 'meta';
        return null;
      }
    }

    // Bare "yyyy/mm/dd hh:mm:ss" timestamp (nginx error log's own line
    // prefix, outside any brackets).
    if (stream.match(/^\d{4}\/\d{2}\/\d{2} \d{2}:\d{2}:\d{2}/)) return 'meta';

    if (stream.match(/^[A-Za-z0-9][\w.-]*/)) {
      const word = stream.current();
      const precedingChar = stream.string[stream.start - 1];

      if (state.expectStatus) {
        state.expectStatus = false;
        if (/^\d{3}$/.test(word)) return statusTag(word);
      }

      const lowerWord = word.toLowerCase();
      if (lowerWord === 'up') return 'string';
      if (lowerWord === 'down') return 'invalid';

      // A dotted quad is only trusted as an IP when it isn't immediately
      // after a single '/' (a path segment, or - the far more common case
      // in a real access log - a version number like "Chrome/139.0.0.0" in
      // the User-Agent string, which is structurally identical to an IP).
      // The legitimate "IP right after a URL scheme" case is already
      // handled above via afterScheme before this branch is ever reached.
      if (isDottedIPv4(word) && precedingChar !== '/') return 'number';

      if (word.includes('.') && precedingChar !== '/') {
        const tld = domainTld(word);
        if (tld && KNOWN_TLDS.has(tld)) return 'propertyName';
      }

      if (HTTP_METHODS.has(word.toUpperCase())) return 'keyword';

      return null;
    }

    stream.next();
    return null;
  },
};

const LOG_LANGUAGE = StreamLanguage.define(parser);

export const LOG_LANGUAGE_SUPPORT: Partial<Record<LanguageId, () => LanguageSupport>> = {
  log: () => new LanguageSupport(LOG_LANGUAGE),
};
