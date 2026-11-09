// The row checks `projectHistoryView` reads, on the inputs of Cindy's own tests and `cases.json`.
import fs from 'node:fs';
import path from 'node:path';

import { record } from './record.mjs';

const shared = 'packages/maker-shared/src/__tests__';
const TESTS = [
  ['pluginResults.test.ts', {
    '../payloadSummary': ['extractPayloadToolResultMedia', 'extractPayloadToolResultFiles', 'extractPayloadToolCardIds'],
    '../historyViewProjection': ['hasVisibleHistoryResult'],
  }],
  ['payloadSummary.test.ts', { '../payloadSummary': ['extractPayloadToolResultMedia'] }],
  ['syntheticTrigger.test.ts', { '../syntheticTrigger': ['isContinuationMessage'] }],
  ['messageNormalize.test.ts', { '../messageNormalize': ['parseMessageToolUse', 'messageContentToPreview'] }],
  ['toolUseDescriptor.test.ts', { '../messageNormalize': ['parseMessageToolUse'] }],
  ['agentTask.test.ts', { '../agentTask': ['isAgentTaskToolName'] }],
  ['richAnswer.test.ts', { '../richAnswer': ['hasRichAnswer'] }],
];

export async function rows({ esbuild, tmp, nodePaths, alias, fixtures, write, fns }) {
  const dir = path.join(fixtures, 'history-rows');
  const cases = JSON.parse(fs.readFileSync(path.join(dir, 'cases.json'), 'utf8'));
  const inputs = Object.fromEntries(Object.keys(fns).map((name) => [name, new Map()]));
  const add = (name, args) => {
    const json = JSON.stringify(args);
    // Rust strings hold no lone surrogate.
    if (json !== undefined && !/\\ud[89a-f]/i.test(json)) inputs[name].set(json, JSON.parse(json));
  };
  for (const [file, targets] of TESTS) {
    const test = path.join(tmp, shared, file);
    for (const [name, args] of await record({ esbuild, tmp, nodePaths, alias, test, targets })) add(name, args);
  }
  for (const [name, list] of Object.entries(cases)) for (const args of list) add(name, args);
  const expected = Object.fromEntries(Object.entries(inputs).map(([name, map]) =>
    [name, [...map.values()].map((args) => [args, fns[name](...args) ?? null])]));
  write(path.join(dir, 'expected.json'), expected);
  return Object.values(expected).reduce((n, list) => n + list.length, 0);
}
