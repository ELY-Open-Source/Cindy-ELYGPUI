// Asks V8 in Cindy's Electron for `Intl.Segmenter`'s words: tests/fixtures/word-break/v8.json and resources/word-classes.json.
const fs = require('node:fs');
const path = require('node:path');

const root = path.join(__dirname, '..');
const files = (dir) => fs.readdirSync(dir, { withFileTypes: true }).sort((a, b) => (a.name < b.name ? -1 : 1)).flatMap((entry) =>
  entry.isDirectory() ? files(path.join(dir, entry.name)) : entry.name.endsWith('.json') ? [path.join(dir, entry.name)] : []);
const strings = (dir) => {
  const out = [];
  const walk = (value) => {
    if (typeof value === 'string') out.push(value);
    else if (value && typeof value === 'object') Object.values(value).forEach(walk);
  };
  for (const file of files(dir)) walk(JSON.parse(fs.readFileSync(file, 'utf8')));
  return out;
};
let state = 20261111;
const next = () => {
  state = (state + 0x6d2b79f5) >>> 0;
  let t = Math.imul(state ^ (state >>> 15), 1 | state);
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
};
const pick = (count) => Math.floor(next() * count);
const chars = (codes) => codes.map((code) => String.fromCodePoint(code));
// One character or more of each class the rules name.
const POOL = chars([0x61, 0x42, 0xe9, 0xdf, 0x44f, 0x5d0, 0x5d1, 0x634, 0x31, 0x32, 0x663, 0x2e, 0xff0e, 0x3a, 0xff1a, 0xfe55, 0x2c, 0x3b, 0x27, 0x2019, 0x22, 0xb7,
  0x5f, 0x203f, 0x2d, 0x2f, 0x20, 0x20, 0x3000, 0x9, 0xd, 0xa, 0x85, 0x2028, 0x301, 0x200d, 0xad, 0x200b, 0x2060, 0x1f1fa, 0x1f1f8, 0x1f1ef, 0x1f468, 0x1f469, 0x2764,
  0xfe0f, 0x1f600, 0x1f3fd, 0xd55c, 0xae00, 0x1100, 0x1161, 0x30ab, 0x30bf, 0xff76, 0x30fc, 0x3042, 0x306e, 0x6f22, 0x5b57, 0x3006, 0xe44, 0xe17, 0xe22, 0xe48, 0x1780,
  0x1781, 0xea1, 0x1019, 0x21, 0x28, 0x29, 0x40, 0x23, 0x24, 0x25, 0x2b, 0x3d, 0x7e, 0x201e, 0xab, 0xa0, 0x202f, 0xfeff, 0x0, 0x1c6, 0xfb01, 0x2460, 0xbd, 0xb2, 0x5f3,
  0x5f4, 0x3031, 0x309b, 0x3005, 0x10ef4, 0x1b168, 0x1f7da, 0x1f1ae, 0x378]);
// Characters NFKC changes, and marks of the kana.
const ODD = chars([0xff76, 0xff9e, 0xff9f, 0xff70, 0xff8a, 0xff8b, 0xff66, 0xff71, 0x30fc, 0x309b, 0x309c, 0x3099, 0x309a, 0xf900, 0xfa10, 0xfa30, 0x2f800, 0x3300, 0x337f,
  0x3314, 0x309f, 0x30ff, 0x3005, 0x3007, 0x30fb, 0xff65, 0x32d0, 0x3220, 0x2e80, 0x2f00, 0x3400, 0x20000, 0x30a0, 0x31f0, 0x1b000, 0x3031, 0xff21, 0xff11, 0x3000]);

const texts = [];
const all = strings(path.join(root, 'locales'));
for (let nth = 0; nth < all.length; nth += 16) texts.push(all[nth]);
for (let n = 0; n < 8000; n++) texts.push(Array.from({ length: 1 + pick(9) }, () => POOL[pick(POOL.length)]).join(''));
const cjk = ['ja', 'zh-CN', 'zh-TW'].flatMap((lang) => strings(path.join(root, 'locales', lang)));
for (let n = 0; n < 8000; n++) {
  const from = [...cjk[pick(cjk.length)]];
  const start = pick(from.length);
  const part = from.slice(start, start + 2 + pick(12));
  for (let k = pick(4); k > 0; k--) part.splice(pick(part.length + 1), 0, ODD[pick(ODD.length)]);
  texts.push(part.join(''));
}
const segmenter = new Intl.Segmenter(undefined, { granularity: 'word' });
// A renderer has met Han text before long: ICU keeps the CJK engine from then on.
[...segmenter.segment(String.fromCodePoint(0x6f22, 0x5b57))];
const seen = new Set();
const rows = [];
for (const text of texts) {
  if (seen.has(text) || /[\ud800-\udfff]/.test(text.replace(/[\ud800-\udbff][\udc00-\udfff]/g, ''))) continue;
  seen.add(text);
  rows.push([text, [...segmenter.segment(text)].map((part) => part.index + part.segment.length)]);
}
const out = path.join(root, 'tests', 'fixtures', 'word-break', 'v8.json');
fs.writeFileSync(out, `${JSON.stringify({ versions: { icu: process.versions.icu, unicode: process.versions.unicode, v8: process.versions.v8 }, rows }, null, 0)}\n`);
/** The code points `test` takes, as inclusive ranges. */
const ranges = (test) => {
  const found = [];
  for (let code = 0; code <= 0x10ffff; code++) {
    if ((code >= 0xd800 && code <= 0xdfff) || !test.test(String.fromCodePoint(code))) continue;
    const last = found.at(-1);
    if (last && last[1] === code - 1) last[1] = code;
    else found.push([code, code]);
  }
  return found;
};
// ICU4X's data may be a Unicode version ahead: what V8 holds unassigned, and its pictographs.
const classes = { unicode: process.versions.unicode, unassigned: ranges(/\p{Cn}/u), pictographs: ranges(/\p{Extended_Pictographic}/u) };
fs.writeFileSync(path.join(root, 'resources', 'word-classes.json'), `${JSON.stringify(classes)}\n`);
console.log(`${rows.length} texts, ICU ${process.versions.icu}, Unicode ${classes.unicode}`);
