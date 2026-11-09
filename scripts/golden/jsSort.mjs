// V8's `Array.prototype.sort` on seeded arrays, with `compareMessageTimeline`'s non-total order.
import fs from 'node:fs';
import path from 'node:path';

const SHORT = [4, 5, 6, 7, 8, 9];
const LENGTHS = [0, 1, 2, 3, ...SHORT, ...SHORT, ...SHORT, ...SHORT, 13, 31, 32, 33, 63, 64, 65, 100, 257, 1000, 3000];
const SHAPES = ['sparse', 'same-time', 'nearly-sorted', 'descending'];

const random = (seed) => () => {
  seed = (seed * 1103515245 + 12345) % 2147483648;
  return seed / 2147483648;
};

function pairs(shape, length, next) {
  return Array.from({ length }, (_, i) => {
    const r = next();
    const rowid = r < 0.4 ? null : Math.floor(next() * length);
    if (shape === 'sparse') return [next() < 0.3 ? null : Math.floor(next() * 5), rowid];
    if (shape === 'same-time') return [0, rowid];
    if (shape === 'nearly-sorted') return [next() < 0.05 ? null : i + (next() < 0.1 ? -3 : 0), rowid];
    return [length - i - (next() < 0.2 ? 1 : 0), rowid];
  });
}

// Natural ascending runs of these lengths; equal times with missing rowids break transitivity.
const RUNS = [[100, 50, 50], [128, 64, 64, 32], [64, 32, 32], [60, 40, 60, 40, 60], [200, 100, 100, 50, 50], [70, 35, 35, 70], [96, 56, 40, 32, 32], [128, 72, 56, 128, 72, 56]];

function runs(lengths, next, block) {
  return lengths.flatMap((length) => Array.from({ length }, (_, i) => [
    Math.floor(i / block), next() < 0.5 ? null : (i % block) * 10 + Math.floor(next() * 10),
  ]));
}

// One time, rowids that skip: insertion and run counting part ways.
const CRAFTED = [
  [[0, null], [0, 12], [0, 35], [0, null], [0, 11], [0, null], [0, 5]],
  [[0, null], [0, 12], [0, 35], [0, null], [0, 11], [0, 40], [0, 5], [0, null]],
];

const compare = (a, b) => {
  const time = (a[0] ?? Infinity) - (b[0] ?? Infinity);
  if (!Number.isNaN(time) && time !== 0) return time;
  const rowid = (a[1] ?? NaN) - (b[1] ?? NaN);
  if (!Number.isNaN(rowid) && rowid !== 0) return rowid;
  return 0;
};

export function jsSort({ fixtures }) {
  const next = random(20261109);
  const cases = SHAPES.flatMap((shape) => LENGTHS.map((length) => {
    const items = pairs(shape, length, next);
    const order = items.map((item, index) => ({ item, index })).sort((a, b) => compare(a.item, b.item));
    return { shape, items, order: order.map(({ index }) => index) };
  })).concat(CRAFTED.map((items) => {
    const order = items.map((item, index) => ({ item, index })).sort((a, b) => compare(a.item, b.item));
    return { shape: 'crafted', items, order: order.map(({ index }) => index) };
  })).concat(RUNS.flatMap((lengths) => [2, 4, 8].map((block) => {
    const items = runs(lengths, next, block);
    const order = items.map((item, index) => ({ item, index })).sort((a, b) => compare(a.item, b.item));
    return { shape: `runs ${lengths.join(' ')} by ${block}`, items, order: order.map(({ index }) => index) };
  })));
  fs.writeFileSync(path.join(fixtures, 'js-sort', 'cases.json'), `${JSON.stringify(cases)}\n`);
  return cases.length;
}
