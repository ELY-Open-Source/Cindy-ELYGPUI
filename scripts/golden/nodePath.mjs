// `path` of the Node in Cindy's Electron, on crafted paths and seeded mixes of their parts.
import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';
import path from 'node:path';

const PARTS = ['a', 'b c', '..', '.', '...', '/', '\\', '/', '\\', '//', 'C:', 'c:', 'z:x', 'CON', 'con:', 'COM1:', 'LPT\xb9:', 'NUL', 'aux:', '?', 'server', 'share', '\u00e9', '\u{1f600}', ':', 'CONx', '~'];
const CRAFTED = ['', '.', '/', '\\', 'a', '\u00e9', '\u{1f600}', 'CONx', 'CON\u{1f600}', 'NULL', '//server/share', '//server/share/', '\\\\server\\share\\x\\..\\..', '\\\\?\\C:\\a', '\\\\.\\COM1:\\x', '//./CON:', '///a', '//a', 'C:', 'C:a\\..\\..', 'C:\\a\\..\\..', './C:/x', 'a/../C:', 'x/y:/z', 'D:/app.tsx', '/D:/app.tsx'];

const PROBE = `
const path = require('node:path');
const { singles, pairs } = JSON.parse(require('node:fs').readFileSync(0, 'utf8'));
const one = (fn) => singles.map((value) => [value, fn(value)]);
const two = (fn) => pairs.map((parts) => [parts, fn(...parts)]);
process.stdout.write(JSON.stringify({
  posixNormalize: one(path.posix.normalize), posixBasename: one(path.posix.basename), posixJoin: two(path.posix.join),
  win32Normalize: one(path.win32.normalize), win32Basename: one((value) => path.win32.basename(value)),
  win32Absolute: one(path.win32.isAbsolute), win32Join: two(path.win32.join),
}));`;

const random = (seed) => () => {
  seed = (seed * 1103515245 + 12345) % 2147483648;
  return seed / 2147483648;
};

export function nodePath({ cindy, fixtures, write }) {
  const next = random(96);
  const text = () => Array.from({ length: Math.floor(next() * 7) }, () => PARTS[Math.floor(next() * PARTS.length)]).join('');
  const singles = [...new Set([...CRAFTED, ...Array.from({ length: 3000 }, text)])];
  const pairs = [...CRAFTED.map((value) => ['/work/dir', value]), ...CRAFTED.map((value) => ['C:\\work', value]),
    ...Array.from({ length: 2000 }, () => [text(), text()])];
  const electron = createRequire(path.join(path.resolve(cindy), 'package.json'))('electron');
  const out = JSON.parse(execFileSync(electron, ['-e', PROBE], {
    env: { ...process.env, ELECTRON_RUN_AS_NODE: '1' }, maxBuffer: 1 << 28, input: JSON.stringify({ singles, pairs }),
    stdio: ['pipe', 'pipe', 'ignore'],
  }));
  write(path.join(fixtures, 'node-path', 'expected.json'), out);
  return Object.values(out).reduce((n, list) => n + list.length, 0);
}
