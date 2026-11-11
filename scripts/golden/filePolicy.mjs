// `buildSensitiveMediaBlocklist`, in the Node of Cindy's Electron, on crafted and seeded hosts.
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';

const KEYS = ['SystemRoot', 'ProgramFiles', 'ProgramFiles(x86)', 'ProgramData', 'LOCALAPPDATA', 'APPDATA', 'CLOUDSDK_CONFIG',
  'GH_CONFIG_DIR', 'GNUPGHOME', 'DOCKER_CONFIG', 'AZURE_CONFIG_DIR', 'AWS_SHARED_CREDENTIALS_FILE', 'AWS_CONFIG_FILE',
  'AWS_WEB_IDENTITY_TOKEN_FILE', 'GOOGLE_APPLICATION_CREDENTIALS', 'AZURE_FEDERATED_TOKEN_FILE', 'NPM_CONFIG_USERCONFIG',
  'npm_config_userconfig', 'REGISTRY_AUTH_FILE', 'KUBECONFIG', 'CARGO_HOME', 'XDG_RUNTIME_DIR', 'XDG_CONFIG_HOME',
  'CHROME_CONFIG_HOME', 'CHROME_USER_DATA_DIR'];
const POSIX = {
  homes: ['/home/u', '/Users/u', '', '/home/u/', '/data/../home/u', 'rel/home', '/'],
  folders: ['/work/dir', '/', '/home/u/project'],
  values: ['/opt/x', '/home/u/.config', 'rel/dir', '/a/../b', '/tmp/k1:/tmp/k2', '/tmp/a;/tmp/b', ':/tmp/k::', '', '/', '/vol/home/u/x/',
    '/home/u/.config/', '.'],
  links: [['/home', '/vol/home'], ['/etc', '/private/etc'], ['/Users/u', '/System/Volumes/Data/Users/u'], ['/var', '/private/var'],
    ['/etc', ''], ['/home/u/.config', '/home/u/.config']],
  exists: ['/', '/vol/home', '/private/etc', '/System/Volumes/Data/Users', '/private', '/home', '/Users', '', '/vol/home/u/.ssh', '/opt'],
};
const WIN32 = {
  homes: ['C:\\Users\\u', 'c:/Users/u', '', '\\\\server\\share\\u', 'D:Users', '\\Users\\u', 'C:\\'],
  folders: ['C:\\work\\dir', 'D:\\', '\\\\server\\share\\work'],
  values: ['D:\\Data', 'E:/x/../y', 'rel\\dir', 'C:\\a;D:\\b', 'C:\\a:b', ';D:\\k;;', '', '\\\\srv\\sh\\cfg', '\\rooted', 'D:drive-rel',
    'C:\\Windows', 'c:\\windows\\', '.'],
  links: [['C:\\Users', 'D:\\Users'], ['C:\\Windows', 'C:\\WINDOWS'], ['\\\\server\\share', 'E:\\share'], ['D:\\', 'F:\\']],
  exists: ['C:\\', 'D:\\', 'D:\\Users', 'E:\\share', 'F:\\', 'C:\\Users\\u', '\\\\srv\\sh\\'],
};
// The hosts of Cindy's own tests.
const CRAFTED = [
  { platform: 'linux', home: '/home/u', cwd: '/', env: {}, links: [], exists: [] },
  { platform: 'linux', home: '/home/u', cwd: '/', env: { XDG_CONFIG_HOME: '/data/cfg', CHROME_CONFIG_HOME: '/opt/chrome-cfg' }, links: [], exists: ['/'] },
  { platform: 'darwin', home: '/Users/u', cwd: '/', env: {}, links: [['/etc', '/private/etc'], ['/Users/u', '/System/Volumes/Data/Users/u']],
    exists: ['/private/etc', '/System/Volumes/Data/Users/u'] },
  { platform: 'linux', home: '/home/u', cwd: '/', env: {}, links: [['/home', '/vol/home']], exists: ['/vol/home'] },
  { platform: 'win32', home: 'C:\\Users\\u', cwd: 'C:\\', env: {}, links: [], exists: [] },
  { platform: 'win32', home: 'C:\\Users\\u', cwd: 'C:\\', env: { LOCALAPPDATA: 'D:\\AppData\\Local', APPDATA: 'D:\\AppData\\Roaming',
    KUBECONFIG: 'C:\\k\\a;D:\\k\\b' }, links: [], exists: ['C:\\', 'D:\\'] },
  { platform: 'freebsd', home: '/home/u', cwd: '/', env: { KUBECONFIG: '/k/a:/k/b', CARGO_HOME: '/opt/cargo' }, links: [], exists: [] },
];

const PROBE = `
const { bundle, cases } = JSON.parse(require('node:fs').readFileSync(0, 'utf8'));
const { buildSensitiveMediaBlocklist } = require(bundle);
// A file system of words: the first link over a path maps it, and it is there under a root that exists.
const real = ({ platform, links, exists }) => (value) => {
  const sep = platform === 'win32' ? '\\\\' : '/';
  const under = (root, at) => at === root || at.startsWith(root.endsWith(sep) ? root : root + sep);
  const link = links.find(([from]) => under(from, value));
  const at = link ? link[1] + value.slice(link[0].length) : value;
  if (!exists.some((root) => under(root, at))) throw new Error('ENOENT');
  return at;
};
process.stdout.write(JSON.stringify(cases.map((host) => {
  process.cwd = () => host.cwd;
  return [host, buildSensitiveMediaBlocklist({ platform: host.platform, homeDir: host.home, env: host.env, realpathSync: real(host) })];
})));`;

// mulberry32.
const random = (seed) => () => {
  seed = (seed + 0x6d2b79f5) | 0;
  let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
};

export async function filePolicy({ esbuild, tmp, nodePaths, alias, fixtures, write }, cindy) {
  const bundle = path.join(tmp, 'file-policy.cjs');
  await esbuild.build({ entryPoints: [path.join(tmp, 'apps/desktop/src/main/filePathPolicy.ts')], bundle: true, format: 'cjs',
    platform: 'node', outfile: bundle, logLevel: 'error', nodePaths, alias });
  const next = random(963);
  const pick = (list) => list[Math.floor(next() * list.length)];
  const some = (list, share) => list.filter(() => next() < share);
  const mixed = () => {
    const platform = pick(['linux', 'linux', 'darwin', 'win32', 'win32']);
    const host = platform === 'win32' ? WIN32 : POSIX;
    const env = Object.fromEntries(some(KEYS, 0.3).map((key) => [key, pick(host.values)]));
    return { platform, home: pick(host.homes), cwd: pick(host.folders), env, links: some(host.links, 0.4), exists: some(host.exists, 0.4) };
  };
  const cases = [...CRAFTED, ...Array.from({ length: 700 }, mixed)];
  const electron = createRequire(path.join(path.resolve(cindy), 'package.json'))('electron');
  const out = JSON.parse(execFileSync(electron, ['-e', PROBE], {
    env: { ...process.env, ELECTRON_RUN_AS_NODE: '1' }, maxBuffer: 1 << 28, stdio: ['pipe', 'pipe', 'inherit'],
    input: JSON.stringify({ bundle, cases }),
  }));
  fs.mkdirSync(path.join(fixtures, 'file-policy'), { recursive: true });
  write(path.join(fixtures, 'file-policy', 'expected.json'), out);
  return out.length;
}
