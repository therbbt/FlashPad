import { StreamLanguage, LanguageSupport, type StreamParser } from '@codemirror/language';
import type { LanguageId } from '../utils/languageDetect';

// Hand-rolled CodeMirror "simple mode" (StreamLanguage, the same lightweight
// per-line token scanner CodeMirror 5 modes used) for the four network
// device config dialects languageDetect.ts recognizes - there's no real
// grammar/parser for these vendor CLIs to build a proper Lezer language
// from, and command vocabularies are far too large (and version-dependent)
// to hardcode a complete keyword dictionary per vendor. Instead: the first
// word of every line is colored as a keyword (in these CLIs it's always the
// command/context verb - interface, vlan, description, switchport, configure,
// admin-state, ...), known interface/port-name shapes get their own color,
// and a small shared set of enable/active-style state words gets another -
// which covers exactly what's visually useful (structure + state) without
// pretending to understand the full grammar.
interface NetworkConfigDialect {
  commentChar: '#' | '!';
  interfacePattern: RegExp;
}

const DIALECTS: Record<'cisco-ios' | 'huawei-vrp' | 'nokia-sros' | 'adva', NetworkConfigDialect> = {
  'cisco-ios': {
    commentChar: '!',
    interfacePattern: /^(GigabitEthernet|TenGigabitEthernet|FastEthernet|HundredGigE|TenGigE|Port-channel|Vlan|Loopback|Tunnel|Serial|Ethernet)\d/i,
  },
  'huawei-vrp': {
    commentChar: '#',
    interfacePattern: /^(GigabitEthernet|XGigabitEthernet|10GE|40GE|100GE|Eth-Trunk|Vlanif|Serial|Ethernet)\d/i,
  },
  'nokia-sros': {
    // Port/interface IDs are bare slot/mda/port numbers (e.g. "1/1/1"), not
    // a named prefix - the only vendor here where that's true.
    commentChar: '#',
    interfacePattern: /^\d+(\/\d+){1,3}$/,
  },
  adva: {
    commentChar: '#',
    interfacePattern: /^(net|acc)-[\d-]+$/i,
  },
};

// Shared across all four dialects rather than per-vendor - it's the same
// networking vocabulary everywhere, not a vendor-specific command set.
const STATE_WORDS = new Set([
  'enable', 'enabled', 'disable', 'disabled', 'active', 'inactive',
  'up', 'down', 'in-service', 'out-of-service', 'shutdown', 'no-shutdown',
  'permit', 'deny', 'allow', 'block', 'on', 'off', 'true', 'false',
]);

interface DialectState {
  firstWordDone: boolean;
}

function buildParser(dialect: NetworkConfigDialect): StreamParser<DialectState> {
  return {
    startState: () => ({ firstWordDone: false }),
    token(stream, state) {
      if (stream.sol()) state.firstWordDone = false;
      if (stream.eatSpace()) return null;

      // Comment markers only count at the start of a line (a stray '#' or
      // '!' inside a value, e.g. a password, isn't a comment).
      if (!state.firstWordDone && stream.peek() === dialect.commentChar) {
        stream.skipToEnd();
        return 'comment';
      }

      if (stream.peek() === '"') {
        stream.next();
        while (!stream.eol() && (stream.peek() as string) !== '"') stream.next();
        if (!stream.eol()) stream.next();
        return 'string';
      }

      // IPv4 address, optionally with a /prefix (also covers dotted-quad
      // subnet masks like 255.255.255.0).
      if (stream.match(/^\d{1,3}(?:\.\d{1,3}){3}(?:\/\d{1,2})?/)) return 'number';

      // Backslash line continuation (ADVA-style multi-line commands).
      if (stream.match(/^\\\s*$/)) return null;

      if (stream.match(/^[A-Za-z0-9][\w.\-/:]*/)) {
        const word = stream.current();
        const isFirstWord = !state.firstWordDone;
        state.firstWordDone = true;
        if (isFirstWord && /^[A-Za-z]/.test(word)) return 'keyword';
        if (dialect.interfacePattern.test(word)) return 'typeName';
        if (STATE_WORDS.has(word.toLowerCase())) return 'atom';
        if (/^\d+$/.test(word)) return 'number';
        return null;
      }

      stream.next();
      return null;
    },
  };
}

const LANGUAGES = {
  'cisco-ios': StreamLanguage.define(buildParser(DIALECTS['cisco-ios'])),
  'huawei-vrp': StreamLanguage.define(buildParser(DIALECTS['huawei-vrp'])),
  'nokia-sros': StreamLanguage.define(buildParser(DIALECTS['nokia-sros'])),
  adva: StreamLanguage.define(buildParser(DIALECTS.adva)),
} as const;

export const NETWORK_CONFIG_LANGUAGE_SUPPORT: Partial<Record<LanguageId, () => LanguageSupport>> = {
  'cisco-ios': () => new LanguageSupport(LANGUAGES['cisco-ios']),
  'huawei-vrp': () => new LanguageSupport(LANGUAGES['huawei-vrp']),
  'nokia-sros': () => new LanguageSupport(LANGUAGES['nokia-sros']),
  adva: () => new LanguageSupport(LANGUAGES.adva),
};
