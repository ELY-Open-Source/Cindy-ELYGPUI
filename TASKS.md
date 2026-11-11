# Tasks

Progress truth, in `docs/PRD.md` chapter order. A task is ticked after review passes, then committed
and pushed. Reviews: `tasks/reviews.md`, at most three rounds per task. Chapters 1 to 8 are done,
listed in `tasks/done.md`.

## 9. Chat view

- [x] 9.0 Expand from Cindy's code
  - 9.0 plan, from `features/cc-agent/CCAgentSessionView.tsx`, `SessionContentHeader.tsx` and
    `NewMakerDraftRoute.tsx`; `lib/makerChatStore.ts` (the session slice, `enterView`,
    `loadOlderMessages`), `maker-shared`'s `historyView*.ts`, `messageWindow.ts`,
    `messageRender.ts`, `messageNormalize.ts`, `richAnswer.ts`, `permissionMode.ts` and
    `agentInputProjection.ts`; main's `localDb/ipc/messages.ts` with `historyViewReader.ts` and
    `historyViewOutline.ts`; `components/chat/` and `components/new-chat/`; `docs/rich-answers.md`,
    `design-rules/message-link-clickability.md` and `attachment-preview.md`. A local task reads
    `local-db:messages:view` (`createRemoteHistoryView` takes it whenever the preload offers
    `historyView`), never the flat list. This chapter draws what the database holds and every
    control the user touches; what needs a running agent (a send reaching the engine, stop, steer,
    a prompt's answer, live streaming) is drawn here and wired with 10.0, which says so in each
    case. Banners and chips owned by later chapters come with them: the provider, upgrade and
    spend banners and the context ring with 11.0 and 13.0, teammates and bot views with 14.0,
    schedule banners with 15.0, plugin prompts and Ghost cards with 16.0, MCP App cards with 17.0,
    worktree and turn-change actions with 18.0, remote and shared-task views with 19.0 and 20.0,
    read aloud and voice with 22.0; the Cindy Make gates stay off (`CINDY_MAKE_ENABLED`). Mermaid,
    KaTeX and highlight.js have no gpui counterpart: their tasks choose the renderer from evidence
    that it matches Cindy's output, and record the choice here
- [x] 9.1 The history read as `local-db:messages:view` and `local-db:messages:work-details` serve
  it: `historyViewReader.ts`'s pages (`HISTORY_VIEW_PAGE_BYTES`, `HISTORY_VIEW_PAGE_ITEMS`, the
  scan limits), `historyViewOutline.ts`'s outline rows, `projectHistoryView`'s items and work
  summaries, the cleared and rewound cuts, `restoreTaskImageRows`, rows read as `messageToCamel`
  reads them; on the database worker, checked against goldens from `sync-golden.mjs`
  - [x] 9.1a The row checks `projectHistoryView` reads: `messageContentToPreview`,
    `parseMessageToolUse` in full, `isOrcaCommunicationTool`, `isAgentPlanToolName`,
    `isAgentTaskToolName`, `isDeliveryProseText` (`hasRichAnswer`), `isSteerUserRow`,
    `isContinuationMessage`, `parseToolResultPayload` with `extractPayloadToolResultMedia`,
    `extractPayloadToolResultFiles` and `extractPayloadToolCardIds`, and `describeToolUse`'s
    `mcp` kind with `createdPath`; golden from Cindy's functions
  - [x] 9.1b `groupWorkRuns` (`workRunGrouping.ts`), `historySubagentScopes`, `projectHistoryView`
    with and without lazy details, `completesHistoryTurn`; golden from Cindy's functions
  - [x] 9.1c Rows: `readMessagesList`'s query (`before`, `beforeTs` and `after` cursors on
    `createdAt` and rowid, cleared and rewound cuts) full and outlined (`historyOutlineContent`,
    `historyOutlineContentStamp`), `messageToCamelWithRowid`, and the reader's `anchor`, `hydrate`,
    `validate`, `revision` on the database worker; tests on Cindy's schema
  - [x] 9.1d `withHistoryArtifacts`: dropped agent meta, `extractCommandOutputPathCandidates` with
    its cache, `createdPathsFromDescriptor`, the `command`, `file` and `fileChange` kinds; golden
  - [x] 9.1e `historyViewReader.ts`'s `page` (boundaries, safe starts, item and byte targets, scan
    budgets, sealed-answer context, outline then hydrate, revisions, in-flight share) and
    `details` (subagent, live and plain ranges, detail cache); tests from `historyViewReader.test.ts`.
    Work lines and page shapes come with 19.0, the first page's import with 10.0,
    `restoreTaskImageRows` and `recoverGeneratedImagesForHistory` with 9.13, legacy turn costs with 9.11
- [x] 9.2 The session slice as `makerChatStore.ts` keeps it for a local task: the first page on
  `enterView`, older pages (`loadOlderMessages`), work details on demand, the reload on a database
  change (the watcher of 8.3), the soft eviction of idle sessions and the window trim;
  `mapServerMessages`
  - [x] 9.2a `HistoryViewController` (`historyViewController.ts`) with `historyView.ts`'s
    `retainHistoryViewPrefix`, `readContinuousHistoryPage` and `readHistoryWorkDetails`: latest
    and older pages, details on expand and `retainExpanded`, the collapsed detail trim,
    `invalidate`'s 500ms coalesce, `locate`, `setActive`, `reset`, the latest-read stamps, over a
    `Transport` (9.2c's local one reads through 9.1e's reader); tests from Cindy's controller
    tests. The `expanded` intent, network state, disk cache and cache receipts come with 20.0
  - [x] 9.2b `mapServerMessages`: `ChatMessage` as the store builds it from rows (hidden
    thinking rows, timeline order, the tool, ask, thinking, plan, error and card branches,
    `projectLegacyUserTurnCosts`); golden from Cindy's function, exported in the bundle only
  - [x] 9.2c `mergeMessages` with `hydratePersistedMessage`, `sortMessagesChronologically` and
    `collapseConsecutiveAutoResumeRows`, and `remoteUserHandoff.ts`'s `confirmRemoteUsers`;
    golden from Cindy's functions, exported in the bundle only
  - [x] 9.2d `createRemoteHistoryView`'s local transport: one reader per database on its
    worker (`local-db:messages:view` with `lazyDetails`, `work-details`), rows mapped by
    `mapServerMessages` into `HistoryChatMessage` (`mapHistoryViewMessages`), a thrown map an
    error of the read
  - [x] 9.2e The store's slice for a local task: `ensureInitialMessages` through the view,
    `enterView` and `leaveView` (ref-counted views), the snapshot's mirror into `messages`
    (borrowed detail rows retired; durable tool pairs come with 10.0's echoes),
    `loadOlderMessages` through the view, the database watcher's `invalidate`
  - [x] 9.2f The raw reads beside the view: `listMessagesFor` and `runRemoteReconcile` (the forced
    read's fallback when a detail range is incomplete), `loadOlderMessages`' raw pages before the
    view is ready, the window's islands they keep; the viewport's load budget comes with 9.14,
    `getSessionFor`'s seeding comes with the fields it seeds (9.18), idle plan discovery with
    `getLatestMessageTodoState` (9.11), pending interactions with 10.0
  - [x] 9.2g Memory: the soft eviction (budget, the 30s check, leave times), the LRU cap of
    `_evictLruIfNeeded` and `_trimMessagesIfNeeded` with its window islands; the runtime's busy
    guard comes with 10.0, warm reading windows with 9.14's scroll memory
- [!] 9.3 The route `/cc-agent/:sessionId` as `CCAgentSessionView.tsx` lays it out: the content
  header (`SessionContentHeader.tsx`: title and its rename, project chip, vendor, the actions this
  chapter owns), the message column with its width rules, the composer dock; loading, empty, not
  found and archived states; `useTrackLastView` keeps the slot
  - [!] 9.3a The route's task: `CindyMakeSessionGate` for a local task (the list's source, else
    `getSessionFor`; missing or failed draws `SessionEntryError` with Back, failed adds Retry), the
    task as the view resolves it (the database's `get` over the list row, archived ones kept), a
    deleted task purged and left for `/cc-agent`, the store's `ensureInitialMessages`,
    `enterView` and `leaveView` with the route; the provisional draft's redirect comes with 9.15
    - Open (review r3): `sync` resets the gate's `loaded` with each new task, but Cindy's gate
      keeps its one `{id, source}` across task switches; old (read by the gate), then s1 (listed),
      then old deleted and reopened draws Missing where Cindy keeps the content open (visits 1)
  - [x] 9.3b The content header: `SessionContentHeader` in the header slot for the route's task:
    the pin mark, the title (`max-w-[40vw]`, its manual window drag) with the double-click rename
    (`w-72 h-7`), More (`Ellipsis` 15 in `h-7 w-7`) opening `SessionTaskMenu` with the header's
    handlers; device, schedule, tag, git and migration badges come with their chapters
  - [x] 9.3c The column: `useProportionalWidth` (the 914 cap, `compact` under 700), the message
    area at `messageWidth` (rows come with 9.4 to 9.14), the composer dock (the `h-8` fade, the
    `pb-5` zone at `inputPad`) that 9.15 fills; `chat-rail-compact`'s type scale comes with the
    rows it shrinks (9.5), the stream's loading and empty states, its `overlayHeight` padding and
    the dock over scrolled rows (its hit blocking, the wallpaper copy and mask) with 9.14
- [x] 9.4 Render items: `buildRenderItems` and `messageWorkGroups.ts` (messages, tool segments,
  work groups, agent plans and tasks, fork origin, turn changes, generated files, tool media),
  subagent rows left out, `collectTurnFinalAssistantClientIds`; golden from Cindy's functions
  - [x] 9.4a The plan timeline (`messageRender.ts`): `findMessageTodoInsertions`,
    `getLatestMessageTodoState` (`findLatestMessageTodoInsertion` is its `insertion`),
    `extractPlanTodos` with `extractTodos`, `isPlanUserBoundary`; golden.
    `extractTodosFromSourceMessage` serves the mobile app alone
  - [x] 9.4b What a turn's rows yield: `extractToolResultMedia` with `dedupeToolMediaByUrl`,
    `richAnswerMediaSources`, `collectGeneratedFiles` (`generatedFiles.ts`, with
    `extractDocumentArtifactMetadata`) over `createdPathsFromDescriptor`,
    `sourcePathCandidatesFromDescriptor` and `resolveToolFilePath`; golden.
    `extractRenderedMarkdownImageTargets` runs the Markdown pipeline and comes with 9.6
  - [x] 9.4c `buildRenderItems`: its filters, subagent rows left out, the result lookup, the plan
    cards with their window check, the linear build (messages, tool segments with the gap split,
    agent tasks with their persisted status, tool media less what the turn shows inline) and the
    turn's end (generated files, hidden subagent media, `isGeneratedFilesTurnSealed`); golden.
    `turn_changes` comes with its store (18.0), ghost cards with 16.0, the Bot order with 14.0,
    voice rows with 22.0, live task updates and durable subagent status with 10.0, Markdown image
    targets in the inline media with 9.6; the golden leaves out rows with a voice card or a Markdown
    image (`later` in `scripts/golden/renderItems.mjs`) until then
  - [x] 9.4d `messageWorkGroups.ts` over 9.1b's grouping: the item adapter (`renderItemStartMs`,
    `renderItemEndMs`, `messageTs`, `isCompletedAssistantMessage`, the work group it makes),
    `collectTurnFinalAssistantClientIds`, `assistantHasFollowingUserBoundary`, the first and last
    user message ids, `insertForkOriginItem`, `renderItemContainsClientId`; golden. A live task
    update's status, launch receipts and times (`deriveAgentTaskStatus` past the persisted status)
    come with 10.0, ghost cards' times with 16.0, a group's deferred work with 9.4e
  - [x] 9.4e `renderHistoryView` (`historyViewRender.ts`) as `buildMessageStreamRenderProjection`
    calls it for a ready view: chunks built by 9.4c and grouped by 9.4d, placeholders, deferred
    work, unique keys; the routed task's items held for the rows of 9.5 to 9.14; the memo caches of
    9.4b's readers (`getToolResultMetadata`'s LRU, `collectCachedGeneratedFiles`); the store's `Rc`
    rows reach 9.4a to 9.4c's readers, which take `&Js` rows, without a copy of a row. The live
    rows (`projectRemoteUsers`, `HistoryViewHandoff`'s `pending` and `tail`, the session's
    streaming flag) come with 10.0, so the view renders with none; the raw projection of a view not
    yet ready (`buildRaw`, `buildCachedRenderItems`) with 9.14's loading states; `historyAfter`
    with the side chat, which 10.0's expansion places; `reuseGeneratedFilesRenderItems` with 9.13's
    card; the caches' reset on a new data owner with 13.0; the fork marker's place with 9.12
- [!] 9.5 The user message (`UserMessage.tsx`), with `chat-rail-compact`'s type scale (from 9.3c):
  `userMessageDisplayText.ts`, linkify and URL links
  (`message-link-clickability.md`), the collapse (`userMessageCollapse.ts`), attachment chips,
  quote, inline reference and session link chips, the forwarded-request text of 8.7
  - [x] 9.5a The bare URL's end (`urlTextBoundary.ts`): `BARE_HTTP_URL_RE_SOURCE`,
    `clipBareHttpAutolink` with its cuts (unbalanced parentheses, code host notes, path brackets,
    Markdown wraps, IDN dots, trailing junk); golden on the inputs of Cindy's tests
  - [x] 9.5b Linkify (`userMessageLinkify.ts`): `findLinkifyMatches` over URLs, image paths,
    session and project deep links in bare and Markdown form (`deepLink.ts`'s sources and parsers,
    `findMarkdownLabelStart`, `deepLinkSchemes.ts`); golden
  - [x] 9.5c Display text and collapse estimates: `userMessageDisplayText.ts`
    (`parseOrcaCommunicationContent`, `resolveHookGroupContext` with `captureImContext`,
    `resolveUserDisplayText`), `shouldAutoCollapseUserMessageContent` and
    `mayExceedVisualLineThreshold`; golden. An automation message's threshold (4) comes with its
    caller in 9.5e
  - [x] 9.5d The sent text's tokens (`UserMessage.tsx`): `looksLikePath`, `looksLikeCommand`, the
    command and mention split, `buildSentInlineTokens`, `projectSentPastedPlainText`,
    `projectSentInlinePlainText` with `sentAgentReferenceDisplayLabel`,
    `locateChatQuoteTextSegmentStarts`, `projectSentRanges`; maker-shared `chatQuotes.ts`
    (`formatQuoteForSend`, `parseChatQuoteSegments`, `joinChatQuoteTextSegments`,
    `stripChatQuoteMarkerLines`); golden. `readAgentInputReferences` and `slashCommandDisplayLabel`
    come with 9.5i's to 9.5l's chips, the outer `renderContent` with the bubble
  - [x] 9.5e The bubble: the stream's list (`msg-stream-items`) with the user row drawn from 9.4e's
    items, `ChatBubble` with its wallpaper glass fill, the text of an ordinary message (display
    text, quote text joined), the collapse measured in the frame's layout with its toggle
    (`line-clamp-10`, 3 for an automation message at threshold 4), `chat-rail-compact`'s type
    scale. The measure runs before paint, so Cindy's first-frame estimate and remembered results
    have no reader: `shouldAutoCollapseUserMessageContent` goes. The measured text takes its pasted
    and reference labels with 9.5i's to 9.5l's chips; text selection comes with 9.6, the scroll's
    padding and gutter with 9.14, the action bar with 9.16, ghost cards with 16.0
  - [x] 9.5f Where a link opens: the link open preference (`useLinkOpenPreference.ts`:
    `isLocalOpenUrl` with its golden, the two keys, the memo, the legacy key's migration),
    `isSeparateLinkActivation`, and the `shell:open-external` handler on Electron's
    `shell.openExternal` per platform (`xdg-open`, `NSWorkspace`, `ShellExecuteEx` with Chromium's
    `EscapeExternalHandlerValue`). The billing mail and the macOS privacy panes come with their
    callers
  - [x] 9.5g URL links in the text (`UserMessageUrlLink.tsx`, `message-link-clickability.md`): the
    underlined links of an unfolded message, `activateMessageUrl` with its toasts, the open-with
    menu (`useOpenWithMenu.tsx`), the keys of `linkInteraction.ts`. The whole text is read for
    links until 9.5i splits it into parts; the sidebar browser comes with 17.0, so an open there
    fails with Cindy's toast; the mail preview comes with its chapter
  - [x] 9.5h What a file looks like: maker-shared `filePresentation.ts` (`getFileVisualKind`,
    `FILE_VISUAL_GLYPHS`), golden; the file chip of 9.5i draws it
  - [x] 9.5i The inline flow: the bubble's text as rows the port breaks itself (ICU line break
    opportunities over shaped glyphs and fixed-width chips, as gpui's text wraps), so a chip sits
    in a line of text; `InlineReferenceChip`'s inert face with its tip; the legacy command, agent,
    folder and file chips of `renderContentWithoutPastedText` (`sent::parts`, `FileTypeIcon`),
    links in the text parts only; the fold counts the plain text's rows and clamps the drawn ones.
    `UserFileChip`'s click and menu come with the text lightbox (9.13)
  - [x] 9.5j The persisted ranges of `renderContent`: pasted text, slash command and agent
    reference chips (`SentAgentReferenceChip`'s inert face, `readAgentInputReferences`,
    `slashCommandDisplayLabel`), the measured text with their labels. A session or project
    reference keeps the inert face unfolded until 9.5l, a message reference its text until 9.5k and
    9.5l; their clicks come with 9.5m
  - [x] 9.5k Quote chips (`QuoteChip`) between the text islands of a `quotesEncoded` message, with
    `locateChatQuoteTextSegmentStarts` and `projectSentRanges` per island, and a folded message
    reference's quote chip
  - [x] 9.5l The faces of session and project link chips (`SessionLinkChip`, `ProjectLinkChip`), in
    the text and for an unfolded session, project or message reference, with the task's title and
    the message's text read from the database (`resolveSessionMessageText`); image path links'
    face (`UserMessageImageLink.tsx`, `break-all`); Blink's line breaks between Latin-1 letters
    and after spaces in the bubble's flow
  - [!] 9.5m What the link chips do: a session link opens its task (`useSessionLinkTarget`; the
    message anchor's `searchJump` lands with 9.14, a new window with its chapter), a project link
    focuses its project (`requestProjectFocus`), their Tab stops, keys and menu
    (`useLinkContextMenu`: open, copy link); an image path's click and menu come with 9.13
    - Open after round 3 (P2): a pending open is tied to its chip by the drawn line and UTF-8
      byte, where Cindy keys the chip by its text piece and the match's UTF-16 index. While a
      read is on its way and a history read rewrites the same row: a non-ASCII letter before
      the link ends an ask Cindy keeps (`a cindy://session/old` to `é cindy://session/old`),
      and an earlier link that grows keeps an ask Cindy ends (the second chip of
      `cindy://session/a cindy://session/old` after the first becomes `cindy://session/aaaa`).
      `logs/review/9.5m-r3.md`
  - [x] 9.5n File attachments over the bubble: `UserAttachmentChip`'s face (the file's icon, or
    Download for `isSafetyDowngradedAttachment`; `attachmentSafety.ts`, golden `attachment-safety`),
    also for a message with no text; its click and menus come with 9.13
  - [x] 9.5o Image attachments over the bubble: `ChatImageView`'s `user-attached` face read from
    `xdt-image://` and `cindy-media://` (`ChatLoadingImage`'s frame while it loads, the long
    image's preview, `ImageMissingPlaceholder`'s face for a missing or unreadable image); its
    retry, the lightbox and the menu come with 9.13
  - [x] 9.5p Over the bubble: the goal badge and `MessageSourceLabels` (`AutomationOriginBadge`,
    the plugin, group and device labels with their tooltips; `messageSourceLabel.ts`)
  - [x] 9.5q The Orca form: the Lead and Worker card in place of the bubble, its source labels
  - [x] 9.5r The hook form: `HookTaskCard` with its attachments, fold and attached context
- [ ] 9.6 Markdown (`MarkdownRenderer.tsx`) on the vendored mdast: GFM tables, lists, task
  lists, block quotes, links and images (`remarkLocalPathLinks`, `remarkSessionLinks`,
  `remarkHtmlImages`, `remarkTruncateCjkUrls`, `remark-cjk-friendly`), headings with slugs, the
  copy and selection behaviour; `repairStreamingMarkdown` and the streaming chunks
  (`streamingMarkdownChunks.ts`) for rows still being written;
  `extractRenderedMarkdownImageTargets` (from 9.4b), which 9.4c's inline media then reads (its
  golden then takes rows with a Markdown image)
  - [x] 9.6a The parse: the tree `REMARK_PLUGINS`' parse extensions build (remark-gfm with
      `singleTilde: false`, remark-cjk-friendly's attention, remark-math) on the vendored
      markdown-rs, golden `markdown-tree`
  - [x] 9.6b Transformers that read no path: `remarkTruncateCjkUrls`, `remarkHtmlImages`,
      `remarkSessionLinks`
  - [x] 9.6c Local paths in the tree: `remarkBareSpaceLocalImages`, `remarkLocalPathLinks`,
      `remarkPreserveRawLocalDestinations`, with `markdownTarget.ts`'s classes
  - [x] 9.6d What react-markdown hands its renderers: `skipHtml`, the URL transforms, definitions and
      footnotes resolved, `rehype-slug`'s ids, `rehypeFencedCodeMarker`
  - [ ] 9.6e Blocks drawn in an assistant text row (`.msg-markdown`): paragraphs, headings, lists and
      task lists, block quotes, rules, plain code blocks (their chrome with 9.7)
  - [ ] 9.6f Inline content in the flow's rows: emphasis, strong, strikethrough, inline code, breaks,
      the link's face
  - [ ] 9.6g Tables: columns, alignment, the scroll of a wide table
  - [ ] 9.6h Link targets: `MarkdownTargetLink`, `FileTargetChip`, `InlineCodeWithTarget`, the
      session, project and session-card links, in-document anchors
  - [ ] 9.6i Images: `LightboxImage` in line (its lightbox with 9.13) and
      `extractRenderedMarkdownImageTargets` (from 9.4b), which 9.4c's inline media then reads
  - [ ] 9.6j Rows still being written: `repairStreamingMarkdown`, `splitStreamingMarkdownChunks`, the
      throttle and the word fade
  - [ ] 9.6k Selection and copy over the drawn Markdown
- [ ] 9.7 Code blocks: `rehype-highlight`'s highlight.js grammars and classes mapped to the
  theme's syntax tokens, the language label and copy, `DiffView.tsx` and `MarkdownDiffBlock.tsx`
- [ ] 9.8 Math: `remark-math` with `remarkStrictInlineMath` and `math-markdown`, drawn as KaTeX
  draws it, inline and block (`rehypeMathBlockMarker`)
- [ ] 9.9 Diagrams: `MarkdownMermaidBlock.tsx` with `mermaid-autofix`, the source toggle, error
  state and `MermaidLightbox.tsx`
- [ ] 9.10 Rich answers (`RichAnswer.tsx`, `docs/rich-answers.md`): the `cindy-ui` component tree
  from 8.7's `stream.rs` and `values.rs` drawn on Ely (layout, text, stat, chart, svg, media,
  controls, tabs, details), local control state per message, `send` buttons queued for 10.0
- [ ] 9.11 The assistant turn: `AssistantMessage.tsx`, `ThinkingCard.tsx` and `ThinkingText.tsx`,
  `AgentActionRow.tsx` and `AgentActionsBlock.tsx` with the full `describeToolUse` (details,
  paths, diffs, created paths, from 8.8b2), `WorkGroupBlock.tsx` folded and expanded,
  `ToolCallCard.tsx`, `ToolPayloadLightbox.tsx`, `TodoListCard.tsx`, `AgentTaskCard.tsx`;
  `hydrateLegacyUserTurnCosts` for the turn's cost
- [ ] 9.12 System rows: `SystemCard.tsx` (agent switch, compaction, notices), `ErrorMessageCard.tsx`
  with `errorReasonI18n.ts` and `outputLimitI18n.ts`, `HistoryClearedMarker`, `ForkOriginMarker`,
  `InterruptedTurnBanner.tsx`
- [ ] 9.13 Media and artifacts: `ChatImageView.tsx`, `ChatVideoView.tsx`, `ChatAudioCard.tsx`,
  `GeneratedFilesCard.tsx` (`generatedFilesProjection.ts`), `AttachmentCollection.tsx`, the image,
  video, text and file lightboxes, missing-media placeholders; files through `cindy-media://` and
  the open path of 8.10c (`attachment-preview.md`)
- [ ] 9.14 The viewport (`MessageViewport.tsx`, `useMessageViewport.ts`): the render window
  (`RENDER_WINDOW_INITIAL_ITEMS`, first-paint budget), scroll anchoring and restore per session,
  auto follow, older pages on scroll, `JumpToBottomChip`, `NewMessageIndicator`,
  `PrevMessageJumpChip`, `MessageNavRail.tsx`, navigation keys; the search hit's `searchJump`
  lands here (from 8.7, `searchJumpTargeting.ts`); from 9.3c, the loading and empty states, the
  stream's bottom padding from the dock's height (`useComposerOverlayMetrics`), the dock's hit
  blocking over rows, and under a wallpaper its fixed copy or the stream's mask
- [ ] 9.15 The composer (`ChatInput.tsx`, `composerDocState.ts`): the document as Tiptap keeps it
  (paragraphs, hard breaks, list and quote nodes, mention, pasted-text and slash chips), history,
  placeholder, IME and CJK punctuation, the paste pipeline, `SlashCommandPalette.tsx`,
  `AtMentionPanel.tsx`, drafts as `composerDraftStore.ts` keeps them; the New draft route
  (`NewMakerDraftRoute.tsx`) with its folder and computer, the project header's New in this
  folder (from 8.10a) and provisional task creation (from 8.3)
- [ ] 9.16 Message actions (`MessageActionBar.tsx`): copy, copy as image, the selection quote
  button (`SelectionQuoteButton.tsx`, `appendQuoteToDraft` into 9.15's draft), delete
  (`useDeleteMessage.ts`) and fork (`useForkAtMessage.ts`) on the database; edit last and rewind
  preview drawn, resent with 10.0
- [ ] 9.17 Attachments: `ComposerAttachments.tsx`, the local picker, drop (`composerDrop.ts`,
  `globalDropIntercept.ts`), pasted images and paths, staging and its cleanup
  (`chatAttachmentStageCleanup.ts`), `attachment-preview.md`
- [ ] 9.18 Composer controls: `PermissionSelector.tsx` with `permissionMode.ts` and
  `FullAccessConfirmContent.tsx`, plan mode, effort, fast and thinking toggles, the agent select,
  the send button's states (`SendButton.tsx`), the model selector's face (its catalog with 11.0);
  the `ask_user` and `plan_review` rows as history cards (`AskUserQuestionBubble.tsx`,
  `PlanReviewBubble.tsx`, the plan cards). Pending interactions live only in the runtime
  (`getPendingInteractionsForSession` reads `pendingInteractionResolvers`; a permission makes no
  row), so `PermissionPrompt.tsx`, `AskUserQuestionPrompt.tsx` and the plan action panel are drawn
  here against fixtures, and their input, snapshot rebuild and answers come with 10.0
- [ ] 9.19 Queue and interjection, stop and continue: `PendingQueuePanel.tsx` over
  `agentInputQueueSnapshots.ts` and `agentInputProjection.ts` (rows, edit, reorder, steer), the
  stop face, the interrupted-turn continue; drawn from the database, sent with 10.0
- [ ] 9.20 Tab groups (from 6.7): `right-tabs` and the edge groups, as `tabGroups.ts` and
  `ensureTabGroupPane` place them; edge groups parked on non-chat routes other than settings; an edge group's temporary
  maximize and restore without changing the tree (`panelMaximize.tsx`, `LayoutRoot.tsx`; its only
  trigger is `TabGroupPanel.tsx`); the pane placement signals panels read (`panePlacement.tsx`:
  at window top, at window left, fill); the panel registry as `panels/registry.ts` keeps it
  (built-in kinds with their collapse memory, tab group kinds registered, unregistered and
  parked); `right-tabs` collapse per session under `right-sidebar-collapsed:<sessionId>`; the window
  grows and shrinks as the right group opens and closes (`sidebarPresentation.ts`, `sidebarExpansion.ts`, anchored
  widths)
- [ ] 9.21 Detached windows (from 6.10): bounds in `right-sidebar-window-state.json` and the shared
  `sidebar-tab-window-state.json`, restored without maximize or fullscreen; which tabs are detached
  lives only for the process, so restart or crash returns them to the main window; legacy
  `right-sidebar-window-settings.json` deleted; detach, merge back, close and multi-window cascade as
  `sidebar-tab-window/controller.ts` does; `swallowActivationClick` for these windows too

## 10. Agent runtime

- [ ] 10.0 Expand from Cindy's code, with the agent and session `ndjson` log streams and the
  interrupted-task continue from 8.8, and the task row's running icon, draft pencil and right
  status (`SidebarRightStatusIndicator`, from 8.5a)

## 11. Models and providers

- [ ] 11.0 Expand from Cindy's code, with the rename input's AI rename button (from 8.5b1), the task summary a pin starts (from 8.5b2) and `setPinnedCardSummaries` (from 8.5c2a)

## 12. Settings

- [ ] 12.0 Expand from Cindy's code, with the settings route and its takeover (sidebar, chrome
  cluster, peek and non-chat panels hidden without changing the tree, restored on exit), the title
  bar's menu button (`MenuButton.tsx`), Obsidian theme conversion and the theme import button,
  and shortcut rebinding (`AppShortcutStore` writes, `findAppShortcutConflict`, bindable and
  global checks, Electron accelerators, the recording flag)

## 13. Account and region

- [ ] 13.0 Expand from Cindy's code, with official wallpaper downloads (`wallpaper-video.ts`, from
  6.9e2), the mock mode's gate, account-free start and seed (from 7.0), the dev keychain identity
  marker, `cloud` owners' database (8.2 admits only `local-v1`), isolated device id and `--endpoints-cdn` (from 7.1), the sidebar's owner-scoped storage and its legacy claims
  (with `manualProjectOrder` and project drag from 8.5c2e),
  log upload and redaction, and the database admission in `docs/storage.md` (reader lease,
  read-only migration check, `cjk_seg`)

## 14. Teammates

- [ ] 14.0 Expand from Cindy's code, with the sidebar's Teammates section

## 15. Automations

- [ ] 15.0 Expand from Cindy's code, with the sidebar's Automations row and the dev `--passive`
  scheduler with its shared-profile flag (from 7.1)

## 16. Plugins

- [ ] 16.0 Expand from Cindy's code, with the sidebar's Plugins row, plugin entries and arrivals

## 17. Tools and integrations

- [ ] 17.0 Expand from Cindy's code

## 18. Files and git

- [ ] 18.0 Expand from Cindy's code, with the task row's Archive, Unarchive and Delete and the
  two-step archive button (from 8.5b1), and the bulk bar's Archive and Delete (from 8.6d)

## 19. Remote

- [ ] 19.0 Expand from Cindy's code

## 20. Device link

- [ ] 20.0 Expand from Cindy's code

## 21. IM channels

- [ ] 21.0 Expand from Cindy's code, with the passive dev profile's IM announcement gate (from 7.1)

## 22. Desktop extras

- [ ] 22.0 Expand from Cindy's code, with the Windows and Linux first-close prompt and close behavior
  (`mainWindowCloseBehavior.ts`, `windowBehavior.ts`), which hide to the tray
  and `isVisibleVoiceCallMessage` in 9.4c's first filter (its golden then takes voice rows)

## 23. Distribution

- [ ] 23.0 Expand from Cindy's code, with the local database refusal screen's update views and
  Check for Updates button (from 8.2), the Linux packages' GStreamer runtime dependencies
  (`gst-plugins-good`, `gst-libav`) for wallpaper video
- [ ] 23.last Dispatch CI and get all six targets green; run the app on the Windows runner and check
  that maximized windows survive display and DPI changes without Cindy's maximize recovery
