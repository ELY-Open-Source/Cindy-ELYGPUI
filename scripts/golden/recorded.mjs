// Goldens on the inputs Cindy's own tests pass, plus a fixture's `cases.json`.
import fs from 'node:fs';
import path from 'node:path';

import { record } from './record.mjs';

const shared = 'packages/maker-shared/src/__tests__';
const desktop = 'apps/desktop/src';

/** `tests`: test file to `{ specifier: [export names] }`; `fns`: the functions run. */
export async function recorded({ esbuild, tmp, nodePaths, alias, fixtures, write, name, tests, fns }) {
  const dir = path.join(fixtures, name);
  const cases = JSON.parse(fs.readFileSync(path.join(dir, 'cases.json'), 'utf8'));
  const inputs = Object.fromEntries(Object.keys(fns).map((fn) => [fn, new Map()]));
  const add = (fn, args) => {
    const json = JSON.stringify(args);
    // Rust strings hold no lone surrogate.
    if (json !== undefined && !/\\ud[89a-f]/i.test(json)) inputs[fn].set(json, JSON.parse(json));
  };
  for (const [file, targets] of Object.entries(tests)) {
    const test = path.join(tmp, file);
    for (const [fn, args] of await record({ esbuild, tmp, nodePaths, alias, test, targets })) add(fn, args);
  }
  for (const [fn, list] of Object.entries(cases)) for (const args of list) add(fn, args);
  const expected = Object.fromEntries(Object.entries(inputs).map(([fn, map]) =>
    [fn, [...map.values()].map((args) => [args, fns[fn](...args) ?? null])]));
  write(path.join(dir, 'expected.json'), expected);
  return Object.values(expected).reduce((n, list) => n + list.length, 0);
}

export const ROW_TESTS = {
  [`${shared}/pluginResults.test.ts`]: {
    '../payloadSummary': ['extractPayloadToolResultMedia', 'extractPayloadToolResultFiles', 'extractPayloadToolCardIds'],
    '../historyViewProjection': ['hasVisibleHistoryResult'],
  },
  [`${shared}/payloadSummary.test.ts`]: { '../payloadSummary': ['extractPayloadToolResultMedia'] },
  [`${shared}/syntheticTrigger.test.ts`]: { '../syntheticTrigger': ['isContinuationMessage'] },
  [`${shared}/messageNormalize.test.ts`]: { '../messageNormalize': ['parseMessageToolUse', 'messageContentToPreview'] },
  [`${shared}/toolUseDescriptor.test.ts`]: { '../messageNormalize': ['parseMessageToolUse'] },
  [`${shared}/agentTask.test.ts`]: { '../agentTask': ['isAgentTaskToolName'] },
  [`${shared}/richAnswer.test.ts`]: { '../richAnswer': ['hasRichAnswer'] },
};

const window = { '@cindy/maker-shared/message-window': ['projectHistoryView'] };
export const VIEW_TESTS = {
  [`${shared}/historyView.test.ts`]: { '../historyViewProjection': ['projectHistoryView'] },
  [`${shared}/historyViewOffline.test.ts`]: { '../historyViewProjection': ['projectHistoryView'] },
  [`${desktop}/main/device-link/__tests__/sharedTaskDispatch.test.ts`]: window,
};
