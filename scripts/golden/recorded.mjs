// Goldens on the inputs Cindy's own tests pass, plus a fixture's `cases.json`.
import fs from 'node:fs';
import path from 'node:path';

import { record } from './record.mjs';

const shared = 'packages/maker-shared/src/__tests__';
const desktop = 'apps/desktop/src';

/** `tests`: test file to `{ specifier: [export names] }`; `fns`: the functions run; `keep` filters inputs. */
export async function recorded({ esbuild, tmp, nodePaths, alias, define, fixtures, write, name, tests, fns, keep }) {
  const dir = path.join(fixtures, name);
  const cases = JSON.parse(fs.readFileSync(path.join(dir, 'cases.json'), 'utf8'));
  const inputs = Object.fromEntries(Object.keys(fns).map((fn) => [fn, new Map()]));
  const add = (fn, args) => {
    if (keep && !keep(fn, args)) return;
    const json = JSON.stringify(args);
    // Rust strings hold no lone surrogate.
    if (json !== undefined && !/\\ud[89a-f]/i.test(json)) inputs[fn].set(json, JSON.parse(json));
  };
  for (const [file, targets] of Object.entries(tests)) {
    const test = path.join(tmp, file);
    for (const [fn, args] of await record({ esbuild, tmp, nodePaths, alias, define, test, targets })) add(fn, args);
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

export const ARTIFACT_TESTS = {
  [`${desktop}/renderer/__tests__/generatedFiles.test.ts`]: { '../lib/generatedFiles': ['extractCommandOutputPathCandidates'] },
  [`${desktop}/main/localDb/ipc/__tests__/historyViewOutline.test.ts`]: { '../historyViewOutline': ['withHistoryArtifacts'] },
};

const renderer = `${desktop}/renderer/__tests__`;
export const YIELD_TESTS = {
  [`${renderer}/extractToolResultMedia.test.ts`]: { '@/components/chat/AgentActionRow': ['extractToolResultMedia'] },
  [`${renderer}/generatedFiles.test.ts`]: {
    '../lib/generatedFiles': ['collectGeneratedFiles', 'extractDocumentArtifactMetadata', 'isExplicitFailedToolResult'],
  },
  [`${renderer}/localPathResolver.test.ts`]: { '@/lib/localPathResolver': ['resolveLocalPath', 'resolveToolFilePath'] },
  [`${shared}/messageRender.test.ts`]: { '../messageRender': ['dedupeToolMediaByUrl'] },
};

export const PLAN_TESTS = {
  [`${shared}/messageRender.test.ts`]: {
    '../messageRender': ['findMessageTodoInsertions', 'getLatestMessageTodoState', 'findLatestMessageTodoInsertion', 'extractPlanTodos'],
  },
};

export const BOUNDARY_TESTS = {
  [`${shared}/urlTextBoundary.test.ts`]: { '../urlTextBoundary': ['clipBareHttpAutolinkText'] },
};

export const LINKIFY_TESTS = {
  [`${renderer}/userMessageLinkify.test.ts`]: { '../components/chat/userMessageLinkify': ['findLinkifyMatches'] },
  [`${renderer}/sessionDeepLink.test.ts`]: { '../lib/deepLink': ['parseSessionDeepLinkHref', 'parseProjectDeepLinkHref'] },
};

const displayText = { '@/components/chat/userMessageDisplayText': ['resolveUserDisplayText', 'resolveHookGroupContext'] };
export const USER_MESSAGE_TESTS = {
  [`${renderer}/hookGroupContext.test.ts`]: displayText,
  [`${renderer}/messageNavRailModel.test.ts`]: displayText,
};

export const SENT_TEXT_TESTS = {
  [`${shared}/chatQuotes.test.ts`]: { '../chatQuotes': ['formatQuoteForSend', 'parseChatQuoteSegments', 'stripChatQuoteMarkerLines'] },
};

export const FILE_KIND_TESTS = {
  'packages/maker-shared/src/filePresentation.test.ts': { './filePresentation': ['getFileVisualKind'] },
};

export const ATTACHMENT_TESTS = {
  [`${desktop}/shared/__tests__/attachmentSafety.test.ts`]: { '../attachmentSafety': ['attachmentExtension', 'isDangerousAttachmentName'] },
};

const target = ['classifyInlineCodeTarget', 'decideRemoteLit', 'isAmbiguousPathShape', 'classifyMarkdownLinkTarget', 'looksLikeBareFileReference', 'splitLocalLineSuffix'];
export const LOCAL_PATH_TESTS = {
  [`${renderer}/markdownTarget.test.ts`]: { '../lib/markdownTarget': target },
  [`${renderer}/localPathResolver.test.ts`]: {
    '@/lib/localPathResolver': ['classifyMarkdownHref', 'looksLikeDirectoryPath', 'looksLikeFilePath', 'resolveKnownLocalFileHref'],
    '@/lib/markdownTarget': ['classifyMarkdownLinkTarget'],
  },
};

export const LINK_OPEN_TESTS = {
  [`${desktop}/renderer/hooks/__tests__/useLinkOpenPreference.test.ts`]: { '../useLinkOpenPreference': ['isLocalOpenUrl'] },
};
