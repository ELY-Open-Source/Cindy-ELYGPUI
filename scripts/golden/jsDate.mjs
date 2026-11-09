// `Date.parse` on `js-date/cases.json` in Cindy's Electron, one run per time zone.
import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';
import fs from 'node:fs';
import path from 'node:path';

const ZONES = ['UTC', 'America/New_York', 'Australia/Lord_Howe'];
const PROBE = `const c = JSON.parse(require('fs').readFileSync(process.argv[1], 'utf8'));
console.log(JSON.stringify(c.map((s) => { const v = Date.parse(s); return Number.isNaN(v) ? null : v; })));`;

export function jsDate({ cindy, fixtures, write }) {
  const electron = createRequire(path.join(path.resolve(cindy), 'package.json'))('electron');
  const dir = path.join(fixtures, 'js-date');
  const cases = path.join(dir, 'cases.json');
  const run = (zone) => JSON.parse(execFileSync(electron, ['-e', PROBE, cases], {
    env: { ...process.env, ELECTRON_RUN_AS_NODE: '1', TZ: zone },
  }).toString());
  const version = execFileSync(electron, ['-p', 'process.versions.v8'], {
    env: { ...process.env, ELECTRON_RUN_AS_NODE: '1' },
  }).toString().trim();
  write(path.join(dir, 'expected.json'), { v8: version, zones: Object.fromEntries(ZONES.map((z) => [z, run(z)])) });
  return JSON.parse(fs.readFileSync(cases, 'utf8')).length;
}
