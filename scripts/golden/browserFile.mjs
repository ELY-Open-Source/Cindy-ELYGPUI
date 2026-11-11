// `resolveBrowserFileTarget` and `isBrowserOpenablePath`, in the Node of Cindy's Electron.
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';

const PARTS = ['/', '/', 'tmp', 'a b', '..', '.', '%', '%2f', '%2F', '%5c', '%41', '%E4', '#', '#/s', '?x=1', '?', '\u00e9', '\u{1f600}',
  '.html', '.HTM', '\\', '~', '[', ']', '|', '^', '"', '\t', '\n', ':', 'x', '<', '{', '`', '\u007f', "'", ';', '=', '\u0001', 'C:', '/..'];
// Absolute paths, and values that are neither a path nor a `file:` URL; the URL arm comes with 17.0.
const HEADS = ['/', '/tmp/', '//', '///', '/tmp//', 'tmp/', '', 'https://example.test/', './'];
const WIN_HEADS = ['C:\\', 'c:/', 'Z:\\dir\\', '\\\\server\\share\\', '//server/share/', '\\\\?\\UNC\\server\\share\\', 'C:', 'tmp\\',
  'https://example.test/', '/', '\\', '//', '///', '/\\', '\\/', '//server', '\\\\localhost\\share\\', '\\\\?\\UNC\\LOCALHOST\\s\\',
  '\\\\host:80\\share\\', '\\\\host:\\s\\', '\\\\a\tb\\share\\', '\\\\ser/ver\\share\\', '\\\\ser?x\\s\\', '\\\\ser#x\\s\\',
  '\\\\[::1]\\s\\', '\\\\ho st\\s\\', '\\\\h%41\\s\\', '\\\\\u00e9\\s\\', '\\\\1.2.3\\s\\', '\\\\a@b\\s\\', '\\\\?\\UNC\\\\s\\',
  '\\\\\t\\s\\', '\\\\A.B.\\s\\', '\\\\xn--a\\s\\',
  '//?/C:/', '//./C:/', '\\\\?\\', '\\\\.\\C:\\..\\', '//?/C:/..'];
// The current folder a rooted Windows path with no device resolves on.
const FOLDERS = ['C:\\work\\dir', 'D:\\', '\\\\server\\share\\work', 'c:\\', '\\\\?\\C:\\work',
  '\\\\.\\C:\\work', '\\\\localhost\\share\\folder'];
const CRAFTED = ['', '/', '/tmp/preview page.html', '/tmp/dir/', '/tmp/a/../b.html', '/..', '/tmp/a%2fb.html', 'https://example.test/index.html'];
const WIN_CRAFTED = ['C:\\tmp\\preview page.html', 'C:\\', 'C:\\dir\\', 'C:/a/../b.html', '\\\\server\\share', '\\\\server', '\\\\\\x',
  '\\\\?\\UNC\\server\\share\\x.html', '\\\\?\\UNC\\server', 'C:foo.html', 'https://example.test/index.html'];
const NAMES = ['', 'a', 'a.html', 'a.HTM', 'A.Html', '.html', '.htm', 'a.html.txt', 'a.htmlx', 'dir.html/a', 'dir.html\\a', '/x/y.htm',
  'C:\\x\\y.HTML', 'a.', 'a..html', 'x/.html', 'a.xhtml', 'a.html ', 'a.h\u0131tml', 'a.\u212atml'];

const PROBE = `
const path = require('node:path'), url = require('node:url');
const { bundle, windows, values, folders, names } = JSON.parse(require('node:fs').readFileSync(0, 'utf8'));
const cindy = require(bundle);
if (windows) {
  // Cindy's own code under Windows rules: the host's path and URL calls stand in for win32's.
  Object.assign(path, path.win32);
  const { pathToFileURL } = url;
  url.pathToFileURL = (value) => pathToFileURL(value, { windows: true });
}
const target = (value, at) => {
  if (folders) process.cwd = () => folders[at];
  try { return cindy.resolveBrowserFileTarget(value); } catch { return 'threw'; }
};
process.stdout.write(JSON.stringify({
  target: values.map((value, at) => (folders ? [value, folders[at], target(value, at)] : [value, target(value)])),
  openable: windows ? [] : names.map((name) => [name, cindy.isBrowserOpenablePath(name)]),
}));`;

const random = (seed) => () => {
  seed = (seed * 1103515245 + 12345) % 2147483648;
  return seed / 2147483648;
};

export async function browserFile({ esbuild, tmp, nodePaths, alias, fixtures, write }, cindy) {
  const entry = path.join(tmp, 'apps/desktop/src/browser-file-entry.ts');
  fs.writeFileSync(entry, [
    "export { resolveBrowserFileTarget } from './main/openFileInBrowser';",
    "export { isBrowserOpenablePath } from './shared/browserOpenableExts';",
  ].join('\n'));
  const bundle = path.join(tmp, 'browser-file.cjs');
  await esbuild.build({ entryPoints: [entry], bundle: true, format: 'cjs', platform: 'node', outfile: bundle, logLevel: 'error', nodePaths, alias });
  const next = random(964);
  const pick = (list) => list[Math.floor(next() * list.length)];
  const mix = (heads) => pick(heads) + Array.from({ length: Math.floor(next() * 6) }, () => pick(PARTS)).join('');
  const electron = createRequire(path.join(path.resolve(cindy), 'package.json'))('electron');
  const run = (windows, values, folders) => JSON.parse(execFileSync(electron, ['-e', PROBE], {
    env: { ...process.env, ELECTRON_RUN_AS_NODE: '1' }, maxBuffer: 1 << 28, stdio: ['pipe', 'pipe', 'inherit'],
    input: JSON.stringify({ bundle, windows, values, folders, names: NAMES }),
  }));
  const posix = run(false, [...new Set([...CRAFTED, ...Array.from({ length: 2500 }, () => mix(HEADS))])]);
  const wide = [...new Set([...WIN_CRAFTED, ...Array.from({ length: 4000 }, () => mix(WIN_HEADS))])];
  const win32 = run(true, wide, wide.map(() => pick(FOLDERS)));
  const out = { posix: posix.target, win32: win32.target, openable: posix.openable };
  write(path.join(fixtures, 'browser-file', 'expected.json'), out);
  return Object.values(out).reduce((n, list) => n + list.length, 0);
}
