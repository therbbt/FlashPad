import { StreamLanguage, type StreamParser } from '@codemirror/language';

// Real shell grammars (both @codemirror/legacy-modes/mode/shell, used for
// fenced ```bash blocks, and highlight.js's bash grammar, tried first for
// inline code) only color a small fixed whitelist of known builtins/
// keywords (if/then/for, ls/cd/rm, ...) - verified against actual output:
// "git tag v0.0.0" and "npm install foo" got NO color at all, since "tag"/
// "install" aren't bash keywords and the grammars don't understand "the
// first word is a command name" as a general rule. That's fine for a real
// multi-line bash SCRIPT (still using the real grammar there), but wrong
// for the actual use case of inline code: a one-line invocation of
// whatever CLI tool (git, npm, docker, kubectl, cargo, ...) with args.
//
// This is a generic command-line tokenizer instead: no keyword whitelist,
// so it works the same for every tool. Per token: the first word is always
// the command name; `-x`/`--flag` words are flags; quoted strings are
// strings; anything starting with a digit (or `v` + digit, for versions
// like v0.0.0) past the first word is a number/version; everything else is
// plain text.
export interface CommandLineToken {
  text: string;
  tag: 'keyword' | 'attribute' | 'string' | 'number' | null;
}

export function tokenizeCommandLine(text: string): CommandLineToken[] {
  const tokens: CommandLineToken[] = [];
  let i = 0;
  let firstWordDone = false;
  while (i < text.length) {
    if (/\s/.test(text[i])) {
      const start = i;
      while (i < text.length && /\s/.test(text[i])) i++;
      tokens.push({ text: text.slice(start, i), tag: null });
      continue;
    }
    const isFirst = !firstWordDone;
    if (text[i] === '"' || text[i] === "'") {
      const quote = text[i];
      const start = i;
      i++;
      while (i < text.length && text[i] !== quote) i++;
      if (i < text.length) i++;
      tokens.push({ text: text.slice(start, i), tag: 'string' });
      firstWordDone = true;
      continue;
    }
    const start = i;
    while (i < text.length && !/\s/.test(text[i])) i++;
    const word = text.slice(start, i);
    let tag: CommandLineToken['tag'] = null;
    if (/^-{1,2}[\w-]/.test(word)) tag = 'attribute';
    else if (!isFirst && /^v?\d/.test(word)) tag = 'number';
    else if (isFirst) tag = 'keyword';
    tokens.push({ text: word, tag });
    firstWordDone = true;
  }
  return tokens;
}

// CodeMirror StreamLanguage counterpart of the same rules, for Editor
// mode's inline-code overlay (see markdownCodeLanguages.ts).
const commandLineParser: StreamParser<{ firstWordDone: boolean }> = {
  startState: () => ({ firstWordDone: false }),
  token(stream, state) {
    if (stream.sol()) state.firstWordDone = false;
    if (stream.eatSpace()) return null;
    const isFirst = !state.firstWordDone;

    if (stream.peek() === '"' || stream.peek() === "'") {
      const quote = stream.next();
      while (!stream.eol() && stream.peek() !== quote) stream.next();
      if (!stream.eol()) stream.next();
      state.firstWordDone = true;
      return 'string';
    }
    if (stream.match(/^-{1,2}[\w-][\w.-]*(=\S+)?/)) {
      state.firstWordDone = true;
      return 'attribute';
    }
    if (!isFirst && stream.match(/^v?\d[\w.-]*/)) {
      state.firstWordDone = true;
      return 'number';
    }
    if (stream.match(/^\S+/)) {
      state.firstWordDone = true;
      return isFirst ? 'keyword' : null;
    }
    stream.next();
    return null;
  },
};

export const commandLineLanguage = StreamLanguage.define(commandLineParser);
