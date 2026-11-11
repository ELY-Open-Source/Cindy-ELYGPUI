# Reviews

Each round: task, round, verdict, findings and their fixes. Reviewer: an Orca worker, Claude Code
on Kimi K3.1 Pro (effort high) from 8.7c1a; Codex GPT-6.1 Sol (xhigh) for 8.6 to 8.7b2; Kimi before. Each round's verdict and
gate logs sit in `logs/review/<task>-r<N>*`.

Chapters 1 to 7 sit in `tasks/reviews-done.md`, chapter 8 in `tasks/reviews-done-8.md`.

## 9.0 Chat view plan
- Round 1: FAIL. 9.18 drew the permission prompt from database rows, which do not exist; 9.15's
  selection quote needed 9.16's composer draft.
- Fixed: 9.18 splits history cards (`ask_user`, `plan_review` rows) from pending interactions,
  which live in the runtime and come with 10.0 (panels drawn against fixtures); the composer is
  9.15 and message actions 9.16, the quote landing in 9.15's draft.
- Round 2: PASS.

## 9.1a History row checks
- Round 1: FAIL. The module docs of `message_rows.rs` and `payload/files.rs` ran to two lines.
- Fixed: each is one line; no other new comment spans two.
- Round 2: PASS.

## 9.1b History view projection
- Round 1: FAIL. The History view bullet pushed `AGENTS.md` to 501 lines.
- Fixed: the bullet is two lines shorter; `AGENTS.md` is 499 lines.
- Round 2: PASS.

## 9.1c History rows read
- Round 1: FAIL. `js_date::iso` negated `i64::MIN`; `TASKS.md` reached 503 lines; `rows.rs`'s module doc ran to two lines.
- Fixed: a range check without negation, with tests at both bounds and `i64::MIN`/`MAX`; the split entries are shorter (499 lines); the doc is one line.
- Round 2: PASS.

## 9.1d History artifacts
- Round 1: PASS. Note: a token can start on a separator (`Out-File -FilePath ;out/`), so `tail.terminator < to` is not an equivalent mutant; the code matches Cindy.

## 9.1e History reader
- Round 1: PASS. The in-flight share is left out: reads run one job at a time on the database worker.

## 9.2a History view controller
- Round 1: FAIL. `characters` recursed into deep tool input and overflowed the stack; the reread on a new revision kept `allow_collapsed`, where Cindy's `loadDetails(latest)` checks expansion.
- Fixed: an explicit stack, as `detailCharacters`; the reread passes `false`. Tests: 50,000 nested arrays, and a recovery read over a new revision.
- Round 2: PASS.

## 9.2b mapServerMessages
- Round 1: FAIL. Rust's `sort_by` met Cindy's non-total timeline order (wrong order, a panic at 24 rows); `finishedAt` read ISO only; Cindy Make card arrays spread into objects; prototype tool names did not throw; `String()` never threw.
- Fixed: `js_sort` ports V8's TimSort (insertion under 8); `js_date::parse` ports V8's `DateParser` with local times through jiff; card `data` kept as is; `paramKeys is not iterable`; `js::try_string` at the four coercions, billing sources read before the early return. Goldens `js-sort` (V8) and `js-date` (Cindy's Electron, three zones); the reviewer's inputs join `chat-message`.
- Round 2: PASS.

## 9.2c mergeMessages
- Round 1: FAIL. `valueReadableSize` counts 0 when `JSON.stringify` throws: V8's slow serializer (a `toJSON` key, index keys, cons strings) overflows past about 5,000 levels; the golden script opened with two comment lines.
- Answered: the overflow depth moves with the caller's stack (5655, 5613, 5246 array levels under 0, 100, 1000 frames; `logs/review/9.2c-r2-depth-bisect.log`), and cons strings are invisible to the port, so the throw is not ported. The `String::kMaxLength` throw needs 512 MiB. Comment joined.
- Round 2: FAIL. The stack bound is accepted as a runtime limit; `String::kMaxLength` is fixed: 90M NULs encode to 540,000,002 units and V8 throws.
- Fixed: a field whose encoding passes `(1 << 29) - 24` counts 0; test on the reviewer's input.
- Round 3: PASS.

## 9.2d History view local transport
- Round 1: FAIL. `mapServerMessages` dropped the keys Cindy's literals set to `undefined` (ask, plan review, `toolUseId`, `sharedAuthor*`, `sourceGroup`, `botLearning`, `clientId`), so a merge kept stale ask fields.
- Fixed: the mapper builds `Out`, whose `None` is such a key, in Cindy's place; store rows carry it. Golden `chat-message` records each row's `undefined` keys (a row without a client id added); the reviewer's ask merge is a test.
- Round 2: PASS.

## 9.2e The store's slice for a local task
- Round 1: FAIL. The forced read took its row baseline at first poll, not at call; a two-line doc comment.
- Fixed: `reconcile` captures the baseline, epoch and page read before it spawns; the comment is one line; a regression test changes a row before the queue runs.
- Round 2: PASS.

## 9.2f The raw reads beside the view
- Round 1: FAIL. The raw older pages took the lock at call time but read only while the caller polled, so a dropped waiter left the task busy for good.
- Fixed: the raw read runs on the foreground executor as a shared future; the caller's copy only waits. A test drops the waiter and checks the page lands and the lock lifts.
- Round 2: PASS.

## 9.2g Memory
- Round 1: FAIL. `set` re-inserted a slice the full cache had just pushed out, even when its update was a no-op, so the cache held 21 slices and the access order 20.
- Fixed: `set` reads through `get_or_create` and writes only a real update, as `setState` returns before `sessions.set`; the reviewer's full-cache case is a test.
- Round 2: PASS.

## 9.3a The route's task
- Round 1: FAIL. One read served both the gate and the content, so a deleted task's first read purged it before the visit loaded it again, a content row kept the gate open after the task left list and database, and an error gate left the visit open; two module comments ran two lines.
- Fixed: the gate reads only while the list does not know the task and opens on its own result; the content mounts and unmounts with the gate (its read, then the visit; unmount leaves the store); the deleted exit runs when the content's read lands; comments are one line; the reviewer's three cases are tests.
- Round 2: FAIL. The listed source arriving did not cancel the gate's read, so its late answer left a `loaded` that later hid a missing task; content reads matched only the id, so a read from an earlier mount of the same task purged its remount.
- Fixed: a change of the listed source cancels the gate's read in flight, as the effect's cleanup does; each content read takes a sequence the mount holds, as `createSessionRefreshSequence` does; both cases are tests.
- Round 3: FAIL. Each new task reset the gate's `loaded`, while Cindy's gate keeps its last successful `{id, source}` across task switches; returning to a task the gate read before closes the gate where Cindy keeps it open.
- Left open: marked `- [!]` in TASKS.md.

## 9.3b The content header
- Round 1: FAIL. The title's press was followed only over the title, so a drag that left it moved nothing and a release outside left a press a later hover turned into a move; More was no Tab stop; a route change hid the header's field, which Cindy keeps with its draft; a test comment ran two lines.
- Fixed: the press is followed over the whole window in the capture phase until it moves the window, lifts, or a move comes with no button held; More holds a Tab stop and a press focuses it; the header draws its field whatever task opened it, commits to the task shown now (its stored or shown title is no change) and drops the edit when the header goes; the comment is one line; the reviewer's four cases are tests.
- Round 2: FAIL. Both rename entries looked the task up in any held bucket first, so a stale archived row prefilled the field over the fresh title the header showed.
- Fixed: the double click and More's Rename start from the task the header shows (`header::task`); rows look theirs up in the list; `session_view::task` is gone; the reviewer's two cases are tests.
- Round 3: PASS.

## 9.3c The column
- Round 1: PASS.

## 9.4a The plan timeline
- Round 1: FAIL. A todo's content went through a `String()` that cannot fail, so an object with an own non-callable `toString` became `[object Object]` where Cindy throws a TypeError.
- Fixed: the conversion is `js::try_string`; `extract_plan_todos`, `insertions` and `latest_state` return the throw as `Err`; the golden records a throw's message, with cases for `TodoWrite`, `update_plan` items and plan, arrays, and a subagent row that never converts.
- Round 2: PASS.

## 9.4b What a turn's rows yield
- Round 1: FAIL. `mcpSourceCandidates` read the input in JSON text order where `Object.values` puts integer keys first, ascending; the rich answer's scheme allowlist folded case by Unicode, so `HTTPſ://` passed as `http` where Cindy's `/i` without `u` keeps the address as written.
- Fixed: the candidates come through `js::entries`; the allowlist matches with `(?i-u)`. The golden takes the tool input as JSON text, so its key order reaches both sides, and holds `ſ`, the Kelvin sign and a ligature in schemes.
- Round 2: PASS.
## 9.4c `buildRenderItems`
- Round 1: FAIL. A history artifact with no `ready` came out unready: the file's `ready` took the truthy read where Cindy copies the field and treats only `false` as unready.
- Fixed: the file is unready only for `ready: false`; the exclude checks keep the truthy read. The golden holds an artifact with `ready` absent, `true`, `false`, `0`, `null` and `''`.
- Round 2: PASS.
## 9.4d Work groups over the render items
- Round 1: FAIL. A sealed answer with no `clientId` entered the final-assistant set as the string
  `undefined`, where Cindy's set holds `undefined`.
- Fixed: the set holds `Option<String>`, `None` for a missing id; golden cases for a missing id
  alone, beside the literal id `undefined`, and as an unsealed last answer.
- Round 2: PASS.
## 9.4e `renderHistoryView` for a ready view
- Round 1: PASS.
## 9.5a The bare URL's end
- Round 1: FAIL. P2: `\p{P}`, `\p{Z}` and `\p{L}` came from the `regex` crate's Unicode 16 tables,
  V8's are Unicode 17 (`https://x/a\u{10ED0}b`, `https://x。\u{088F}国中国`). P2: the `url` crate
  refuses `https://ab\u{088F}`, which Node's `new URL` takes, so `[x]` after it stays.
- Fixed: the three classes are V8's, from `resources/url-classes.json`; the reviewer's inputs and
  more Unicode 17 characters are golden cases. The second finding is Node's parser alone: Blink,
  where the message is drawn, refuses that host as the `url` crate does
  (`logs/review/9.5a-r1-url-engines.log`); AGENTS.md records where each engine parts from the
  standard.
- Round 2: PASS.
## 9.5b Linkify
- Round 1: PASS.
## 9.5c Display text and collapse estimates
- Round 1: PASS.
## 9.5d The sent text's tokens
- Round 1: PASS.
## 9.5e The bubble
- Round 1, cut short: the reviewer stopped before its verdict, having named two findings. White
  space at a line's end wrapped into lines Blink never makes, and the fold's button lacked
  `aria-expanded`.
- Fixed before the round ran again: white space at a wrap hangs, as Blink's `pre-wrap` hangs it
  (`logs/review/9.5e-r1-blink-spaces.log`), in the drawn text and the line count alike; the button
  carries `aria-expanded`.
- Round 1, run again: PASS.

## 9.5f Where a link opens
- Round 1: FAIL, then PASS on the fix made while it ran. The old choice's move went on after a
  failed write and removed the old key, where Cindy's `try` leaves it.
- Fixed: `migrate` stops at its first failed read or write, logs it and keeps the old key. One
  mutant survives (the write's error dropped): a test cannot fail the write and pass the removal
  without a file-size limit on the whole process; the reviewer's quota probe covers it
  (`logs/review/9.5f-r1.md`).

## 9.5g URL links in the text
- Round 1: FAIL. The link's Tab stop had the link role and no name, where Cindy's anchor is named
  by its address.
- Fixed: the first hit box carries the address as its `aria-label`. No test in this crate reads the
  accessibility tree, so none covers it.
- Round 2: FAIL. A hit box took its width from the x of its first and last bytes, so a link among
  right-to-left words got a negative width and no click.
- Fixed: `boxes` orders the shaped glyphs by x (the native shaper lists right-to-left text from the
  right) and makes a box per stretch of the link's glyphs on a row; unit tests feed it
  cosmic-text's recorded glyphs for Hebrew and Arabic lines (gpui's test shaper knows no
  direction).
- Round 3: PASS, on the version corrected while the round ran: the first rewrite read the glyphs in
  the shaper's order, which runs from the right in right-to-left text.

## 9.5h What a file looks like

- Round 1: FAIL. P2: the MIME patterns' dot matched as Rust's does, so a CR between `opendocument`
  and `text`, or `vnd` and `rar`, gave Document or Archive where Cindy gives file.
- Fixed: `js_re::regex` refuses a bare dot; the patterns spell it as one UTF-16 unit that is no
  line terminator. Twelve inputs added to the golden (CR, LF, U+2028, U+2029, an astral character,
  a letter).
- Round 2: PASS.

## 9.5i The inline flow

- Round 1: FAIL. P3: an agent chip with an empty name (`@.claude/agents/.md`) showed an empty tip;
  Cindy's `Tip` mounts nothing for empty text.
- Fixed: the chip registers no tooltip for an empty tip; a test hovers such a chip.
- Round 2: PASS.

## 9.5j The persisted ranges

- Round 1: FAIL. P2: a message reference whose `[label](href)` holds a line feed, drawn as text
  until its chips, reached the one-line shaper with the line feed and failed its assertion.
- Fixed: `Reading::plain` splits such words into lines and drops CR; a unit test and a stored
  message cover it.
- Round 2: PASS.

## 9.5k Quote chips

- Round 1: FAIL. P2: a message of quotes alone drew no bubble, since the joined text was empty;
  Cindy shows it when `inlineQuoteCount > 0`.
- Fixed: the row draws when it holds a quote or text; a stored message of two quotes covers it.
- Round 2: PASS.

## 9.5l Link chips' faces

- Round 1: FAIL. P2: the pair table answered for a hyphen before a Latin-1 letter past ASCII,
  where Blink asks the Unicode rules, which read the text before: `x a-é` broke only at the space.
- Fixed: that pair goes to the Unicode rules again, with UAX 14's LB20a, which ICU4X lacks: no
  break after a hyphen that starts a word. A Blink probe of 18 lines is the test
  (`tests/fixtures/line-breaks/hyphen.jsonl`).
- Round 2: PASS.

## 9.5m What the link chips do
- Round 1: FAIL. P2: the latest-ask rule counted asks per mounted content, where Cindy's
  `navigationRequestVersionRef` lives in each chip: another chip's ask ended the first, and a
  folded message's chip still navigated when its read landed.
- Fixed: asks count per chip (`Found.opening` by the chip's element name). A fold ends its
  message's asks at once; a chip the stream no longer draws loses its ask at that render.
- Round 2: FAIL. P2: a chip kept at its place lost no ask when its link's target changed, as a
  history read may rewrite a held row; Cindy's hook ends the ask when the task, the message or
  the device changes, and keys the chip by its place in the text.
- Fixed: each drawn session chip records its line, its byte and its parsed target
  (`Found.marks`); a render that draws the chip with another ends what it asked.
- Round 3: FAIL. P2: the chip's mark is its drawn line and UTF-8 byte, not Cindy's key (the text
  piece and the match's UTF-16 index): a rewritten row can end an ask Cindy keeps, or keep one
  Cindy ends. Left open; the entry is `- [!]`.

## 9.5n File attachments over the bubble

- Round 1: PASS. The reviewer ran Cindy's `attachmentExtension` against the port over every
  Unicode scalar, and laid the chip's styles out in Chromium 148. No live pointer check.

## 9.5o Image attachments over the bubble

- Round 1: FAIL. P2: a failed image whose name holds a line feed panicked in the placeholder's
  one-line measure; Cindy's `truncate` draws it on one line.
- Fixed: the placeholder measures and draws the name through `chip::nowrap`; the tip keeps the
  stored name. Test `a_failed_images_name_draws_on_one_line`.
- Round 2: PASS.

## 9.5p The goal badge and the source labels over the bubble
- Round 1: FAIL. P2: a source task whose id needs escapes did not open: the label navigates to the
  encoded route, as Cindy does, but the router handed the encoded parameter on as the task's id,
  where React Router decodes it. P3: the golden's `expected.json` has 1014 lines.
- Fixed: `router::route` decodes the session parameter (`safelyDecodeURIComponent`: one that does
  not decode stays as written), so `active_session` and the last chat view hold the task's own id.
  Tests `a_task_label_opens_a_task_whose_id_needs_escapes` and the router's. P3 not taken: goldens
  are generated data, and the other goldens run to 400,000 lines.
- Round 2: FAIL. P2: the last chat view must keep what `useTrackLastView`'s `matchPath` gives on
  the raw pathname (`job%201`), not the decoded id. P2: React Router decodes the pathname by
  segment (`decodePath`) before it matches and gives a param's `%2F` back as a slash, so
  `/cc-agent/a%252Fb` is task `a/b`. The fixture's length was dropped as a finding.
- Fixed: `router::route` is `matchRoutes` (`decode_path`, then `matched`); the tracker calls
  `matched` on the pathname as it stands. Router tests hold the values Cindy's own code gave
  (`logs/review/9.5p-r2-route-repro.log`).
- Round 3: PASS.

## 9.5q The Orca form: the Lead and Worker card
- Round 1: FAIL. P3: a message of carriage returns alone drew one line of the code face, where
  Blink's `pre` holds no line box (`logs/review/9.5q-r1-pre-hidden.log`).
- Fixed: the body draws no row for a text of nothing but CR and FF, both 0 high in that probe.
  Test `a_pre_of_ignored_characters_holds_no_line`.
- Round 2: FAIL. P3: a form feed alone on the last line drew a line (`a\n\f` took two), where
  Blink lays out no FF (`logs/review/9.5q-r2-pre.log`).
- Fixed: the body drops CR and FF before it reads its rows; a probe of every C0 control in Cindy's
  Electron shows no other is dropped (`logs/review/9.5q-r3-blink-controls.log`).
- Round 3: PASS.

## 9.5r The hook form: HookTaskCard
- Round 1: FAIL. P2: a context heading broke at the regular weight and drew in medium, so a font
  whose medium runs wider lost a row. P3: the counts added as integers, where Cindy adds JavaScript
  Numbers: 9007199254740991 and two entries read 9007199254740993.
- Fixed: the heading breaks in the style it draws in; the counts stay Numbers through both sums.
  Golden cases hold three sums past the safe range.
- Round 2: FAIL. P3: the finished sum reached the words as an integer, where i18next writes the
  Number: two counts of 9007199254740991 and eight entries read 18014398509481992 for Cindy's
  18014398509481990.
- Fixed: `i18n::Arg::Js` carries a JavaScript Number, which picks its plural form and reads as
  `String(n)`. Golden cases hold three sums past 2^54.
- Round 3: PASS.

## 9.6a The parse: the Markdown tree
- Round 1: FAIL. P2: emphasis and strikethrough resolved in one walk, where Cindy runs two
  resolvers, the first met first (`*~~_a~~b_`). P2: an email found in data an underscore's
  attention had cut (`_中a@b.c_`). P3: `**\0*` lost a star Cindy keeps, since micromark's
  `movePoint` leaves an opener whole at a chunk's end. P3: a four-line comment in the golden script.
- Fixed: `attention::pass` runs each kind apart, in a label attention first, and a matched pair
  resolves what it holds; `chunk_ends` marks the NULs; the domain is read off the source; the
  comment is one line. Three more email rules came out of fresh mixes: an underscore inside an
  email is no attention, `\a` escapes nothing, and no email starts in the one before it. The
  reviewer's 29,513 inputs agree; the golden holds an inline round and an email round.
- Round 2: PASS.

## 9.6b Transformers that read no path
- Round 1: PASS. The reviewer's 39,384 fresh inputs agree on all three transformers; 45 mutants
  killed, one ended in endless recursion, none survived.

## 9.6c Local paths in the tree
- Round 1: FAIL. Two classifier differences among the reviewer's 256,460 fresh calls: an attachment
  with an empty path read as resolved, and inline code with a file shape but no local kind read as
  nothing. All 30,041 trees agreed.
- Round 2: PASS. Both closed: the empty path is filtered as `if (knownPath)` does, and `shape::file`
  is back in `inline_code`. Seeds added for both; 144 mutants killed, none survived.

## 9.6d The renderer tree
- Round 1: FAIL. Two list inputs with a footnote definition in an item got the wrong spread, so
  the renderer tree kept or unwrapped a paragraph Cindy's does not.
- Round 2: FAIL. Both closed, but among the reviewer's 27,208 fresh inputs 259 spreads differed at
  quote, footnote and empty-item boundaries, and a task check after a footnote definition's exit
  stayed text.
- Round 3: PASS. The spread heuristics went: `infer::spreads` replays micromark's exits,
  `subtokenize` and `prepareList` on the port's events. The task check passes a definition's exit
  and an indent after a definition stays outside the paragraph. The reviewer's 27,208 and 59,650
  fresh inputs agree; 614,561 of the author's agree; 47 of 53 mutants killed first, the rest
  removed as redundant code or killed by added inputs.

## 9.6e Markdown blocks
- Round 1: FAIL. A U+FFFC in the text was read as a task check and the row panicked. The
  reviewer's 128 block layouts otherwise matched Blink's heights.
- Round 2: PASS. A row is cut at the bytes the checks were recorded at. All 128 layouts agree
  with Blink, and six probes of checks at row edges pass. No live desktop check was made.

## 9.6f Inline content
- Round 1: FAIL. A paragraph that starts with a hard break panicked (`br` wrote no span), and a
  code holding one space lost its box at a line's end. Heights agreed with Blink in 41 cases.
- Round 2: FAIL. Both closed. New: a code with text and a last space gained a second box, and
  the space before an empty code stayed at a line's end. 136 inputs, no panic.
- Round 3: FAIL. Both closed (`Inline::end` trims a line's tail; 302 cases, 234 agree with
  Blink's CODE boxes). New P1: a soft wrap dropped a code's space and its box or closing
  padding. Fixed after the round with `Look::stub`, not reviewed again. No live desktop check
  was made.

## 9.6g Tables
- Round 1: FAIL. An empty code's box counted as a breakable atom, so a table's minimum was too
  small. The reviewer's other 88 renders agreed with Blink.
- Round 2: FAIL. Closed. New: a box that starts a line, and a code of one space, were left out
  of the minimum; a box after a word's space broke the word before it.
- Round 3: PASS. A space hangs and counts nothing; a code's box with no text joins the atom
  before it, or at a line's start the one after; what is stuck after a hung space hangs too.
  149 earlier renders and 56 new ones agree with Blink but for the test font; the user bubble's
  rows are unchanged over 81,920 comparisons. No live desktop check was made.

## 9.6h1 The workspace path resolver
- Round 1: FAIL. A folder's entries came in the file system's order; Node's `readdir` hands
  them over in byte order, so a capped walk kept other files. 102,925 Node path operations and
  the uncapped resolver requests agreed with Cindy.
- Round 2: PASS. On POSIX the entries sort by their names' bytes, as libuv's `scandir` hands
  them over (libuv 1.52.1 `src/unix/fs.c`; its Windows half does not sort). 139 resolver
  requests agree with Cindy in order, names past ASCII among them. No Windows or macOS run.

## 9.6h2 The renderer's resolver
- Round 1: FAIL. A mounted target read the cache at every draw, so an eviction unlit it and
  asked again; Cindy's hook keeps its state. Stamps were exact instants where Cindy's are whole
  milliseconds. 1026 lookup hrefs agreed.
- Round 2: FAIL. Both closed; 11 hook transitions agreed. New: a waiting hook read the cache at
  its next draw, where Cindy's reads in the callback of the lookup it joined, so a later batch
  or recheck in between changed what it kept.
- Round 3: PASS. `ask` takes the promise's callback, run once the batch of the lookup it
  joined has settled; a hook's run number drops a callback its effect cancelled. The delayed
  eviction and recheck, 501 mounted targets, the millisecond controls, 11 transitions and 1026
  lookup conversions agree with Cindy's hook under React. No live desktop check was made.

## 9.6h3 `MarkdownTargetLink`'s link
- Round 1: FAIL. A link's boxes shared an element id across rows, so only its first row took a
  click; an empty code's box inside a link had no listeners; an authored title was dropped.
  The reveal's extraction kept its behaviour.
- Round 2: PASS. A box is named by row and part, the pill and the wrap's stub carry their link,
  and the title is the link's tooltip. Six reproductions, four regression tests and nine cases
  through Cindy's renderer agree. No live desktop check was made.

## 9.6h4a The faces of a resolved local target
- Round 1: FAIL. A resolved inline-code chip inside an ordinary link took the outer link's click,
  title and Tab stop.
- Round 2: FAIL. A link that starts with a chip took its Tab stop after the chip when prose
  followed: the suffix claimed the anchor's first box before the chip's link box was built.
- Round 3: PASS. A row's link boxes are dressed in document order, so the anchor's stop comes
  before the chips it holds. The reviewer's 37 link tests and Cindy's renderer agree. No live
  desktop check was made.

## 9.6h4b The file menu's main handlers
- Round 1: FAIL. The `@Nx` search split compound extensions at the last dot, `nativeImage`'s
  `NormalizePath` was missing, and `maxBuffer` counted UTF-16 units; a 16-bit PNG rounded where
  Electron cuts.
- Round 2: FAIL. A folder path ending in a separator or a dot sought `folder@2x` beside it, and
  bytes that are no UTF-8 counted one each where Node counts U+FFFD's three.
- Round 3: FAIL. A sequence cut by the end of the output stayed out of `maxBuffer`. Fixed after
  the round, not reviewed again. No live desktop check was made: no file manager opened, no
  clipboard written; macOS and Windows code is cross-linted only.

## 9.6h4c The browser handler
- Round 1: FAIL. A rooted Windows path with no device lost the current folder's drive or share,
  and a UNC host went through `Url::set_host` where Node's hostname setter refuses a port.
- Round 2: FAIL. A `\\?\` current folder read as a UNC share, and a device root lost the
  separator the UNC conversion needs.
- Round 3: PASS. `root` is `path.win32.resolve`'s root parse; the URL's path is written by hand,
  since the `url` crate parts from ada on empty segments and drive letters. No live desktop
  check was made: no browser opened; macOS and Windows code was not cross-compiled this round.

## 9.6h4d The file menu
- Round 1: FAIL. Enter and Space on a chip inside another target stopped at the chip, where
  Cindy's keydown bubbles on and activates the enclosing target too.
- Round 2: PASS. Every box of a target hears the keys and lets Enter and Space bubble. No live
  desktop check was made: the handlers ran on their test recorders.

## 9.6h5a The session and project chips in Markdown
- Round 1: FAIL. The chip's label stayed 12px at every UI font size, and an `em` about the
  link did not slant a session or project chip.
- Round 2: PASS. The label is `--text-12` through `text_size`; an `em` about the link slants
  the chip, the message link's button apart. No live desktop check was made.

## 9.6h5b1 The handoff card in a paragraph
- Round 1: FAIL. A modified Enter or Space did nothing and Enter waited for key up; an empty
  `last_active` read as recently active.
- Round 2: FAIL. A held Space outlived Enter, a click, and a focus gone and back in one update.
- Round 3: PASS. Every activation ends a held Space, and a clean Space clicks through gpui's own
  key-up click. A Space mixed with another key does not see a focus round trip inside one
  update. No live desktop check was made.

## 9.6h5b2 The handoff card in a table's cell
- Round 1: FAIL. The card's min content ignored the cell's `overflow-wrap: anywhere`, and a
  title's raw white space widened its column; a decoded line feed reached the line shaper.
- Round 2: PASS. The badge and the short id count their widest character in the min; the title
  and the id collapse their white space before measure and draw. No live desktop check was made.

## 9.6h5b3 The handoff's route state and its pill
- Round 1: FAIL. The pill's line kept its raw white space: ten spaces widened it and a line
  feed made two lines.
- Round 2: PASS. The drawn line collapses its white space; the route state keeps the title as
  handed on. No live desktop check was made.

## 9.6i1 A text's image targets
- Round 1: FAIL. An origin token whose JSON held a lone surrogate escape failed the port's parse
  and passed as a legacy device id, so an image read as a managed video.
- Round 2: PASS. Each lone surrogate escape reads as U+FFFD before the parse; the fields are read
  for emptiness and `k` alone.

## 9.6i2 The address an image loads
- Round 1: FAIL. A hook whose path stopped being looked up kept the callback of the lookup in
  flight, so a cancelled answer was stored and outlived its file once the cache dropped it.
- Round 2: PASS. A changed path or folder moves the hook's run on; an answer kept earlier
  stays. No live desktop check was made.

## 9.6i3a The image's box in a line
- Round 1: FAIL. The words about a loading frame stood at its bottom, not by its spinner; one
  Retry mended every image of its address; a failed card counted its whole title at least, so
  four cards overflowed a narrow table.
- Round 2: FAIL. Those three closed. A Retry's load was kept by the image's number alone, so a
  changed address drew the old picture; the card's measure left Retry's width out.
- Round 3: FAIL, marked `[!]`. Those two closed. Open: an address that left a slot and came
  back reads the shared failure of its first mount. No live desktop check was made.

## 9.6i3b A local file's bytes
- Round 1: PASS. The reviewer ran Cindy's own handler on 159 requests and the port answered
  each alike; both goldens matched a fresh record; 300,000 path checks found no difference. No
  live desktop check was made; Windows and macOS paths were checked as strings.

## 9.6i3c1 A `data:` address
- Round 1: FAIL. P1: a `data:` SVG's `image` that named a local file was read by gpui's SVG
  renderer, past the sensitive-media list; Blink's image reads no outside reference.
- Round 2: PASS. `svg_image.rs` draws an SVG image with a resolver that reads nothing outside
  it; the reviewer's 38 kinds of outside reference opened no file and reached no server, and
  1,900 addresses load as Blink loads them. No live desktop check was made.

## 9.6i3c2 An `http(s)` address
- Round 1: FAIL. P1: a redirect sent a `Referer`. P2: no CORP check on redirect answers;
  credentials and restricted ports asked; unfollowable redirects drew their body; the address in
  error logs; multi-valued CORP and `Content-Type`; the address not canonical. P3: a 205's body.
- Round 2: FAIL. Those closed. P2: `Content-Type` split at commas inside quotes; fields with
  bytes past ASCII dropped; an empty `Location` followed to itself.
- Round 3: PASS. Fields read as Chromium 148 reads them; 126 loads compared with Blink, no
  difference. No live desktop, outside network, TLS, proxy or cross-platform check was made.

## 9.6j1 Streaming Markdown's repair, chunks and interval
- Round 1: FAIL. P3: the two new fixture files exceed 1000 lines. No functional difference in
  936,964 inputs run through Cindy's functions and the port; the golden regenerated alike.
- Round 2: PASS. AGENTS.md states the exception for data under `tests/fixtures`. No desktop,
  streamed row, timer or word fade was checked: the functions have no caller yet.

## 9.6j2a The word cut under `splitWords`
- Round 1: FAIL. P2: ICU4X's Unicode 18 data gave a class to characters V8's Unicode 17 holds
  unassigned (10,216 of 402,266 texts differed, all with such a character).
- Round 2: PASS. V8's unassigned code points and pictographs come from a table V8 wrote; the
  402,266 texts, 438,924 CJK cases and 14,751 further cases match V8. Stated limits: ICU's
  Thai, Lao, Khmer and Burmese engines are not ported. No desktop or fade check was made.
