# Session view

The routed task's view and a user message's rows, moved from `AGENTS.md`.

`src/session_view.rs` is `CCAgentSessionView` for a local task: `CindyMakeSessionGate`
(a task any held bucket lists opens at once, else `get` reads it; a miss or failure draws
`SessionEntryError`; a Cindy Make source goes back), the task the view holds (the latest read, else
the `active` bucket's row; each watcher reload reads it again), a deleted task purged and left, the
store's `enterView`, `ensureInitialMessages` and `leaveView` with the route. `header.rs` is
`SessionContentHeader` in the header slot (which shows on macOS while it holds one): the pin, the
title (its press followed over the whole window, as pointer capture, with a 4px dead zone before it
moves the window; a double click renames with `rename::header_field`, whose edit outlives a task
switch, commits to the task shown and goes with the header), More (a Tab stop) with `task_menu` under
`Scope::Header`, whose pin writes `pinnedAt` alone. `width.rs` is `useProportionalWidth(914)` and
`column.rs` the view's section: a canvas measures the container and lays the section out in the same
frame (`prepaint_as_root`), the message column at `messageWidth`, the dock (fade, `pb-5` zone, the
composer's row at `inputWidth`) at `inputPad`. `stream.rs` is `MessageStream`'s list: the scroller,
the `pt-7` column at `messageWidth` and `msg-stream-items`, a row per render item; a user's and
an assistant's message draw so far. `user_row.rs` is `UserMessage`'s bubble: the display text (quote text joined),
`ChatBubble`'s user face, and the long message's fold. `attachments.rs` is `UserAttachmentChip`'s
inert face in `fileAttachmentNodes`, over the bubble and for a message with no text: the chips wrap
and stand right, each at most `min(280px, 100%)`, lit under the pointer, the native `title` as gpui's
tooltip; `src/attachment_safety.rs` (`attachmentSafety.ts` with `isSafetyDowngradedAttachment`,
golden `attachment-safety`) picks Download for a file the chat only saves. Its click and menus come
with 9.13. `images.rs` is `ChatImageView`'s `user-attached` face, each image a child of the column
before the files: `src/media_store.rs` resolves `xdt-image://` and `cindy-media://blobs/` as the two
stores' `resolveSafe` do (golden `media-store`, whose Electron stub answers `app.getPath`) and reads
as the protocol handlers' `fs.readFile` does; the read and the decode (by the bytes' own format,
through gpui's `Image`, so a GIF plays) run on the background executor, once per address for the
mounted content (`Content.images`). While it loads `ChatLoadingImage`'s 280 by 180 frame with Ely's
`Spinner` stands in; the image then fades in within 280 by 180 at its ratio and dims to 0.9 under the
pointer. A long image (over three times as tall as wide) draws its square top, cropped at decode
since gpui's `cover` centres, in a 240 box with the hint pill. A read that finds no file draws
`ImageMissingPlaceholder` as missing, any other failure as unavailable; its retry, the lightbox and
the menu come with 9.13. The decode keeps the image's full size; AVIF and animated WebP or PNG are
not decoded as Chromium does. `source_labels.rs` is what stands over those: `MessageSourceLabels`'
row (who sent it, at most one, then the device) and the `/goal` badge. `src/source_label.rs` holds
each label's words, tooltip and route as `AutomationOriginBadge` and the plugin and device badges
give them, golden `source-labels`: `scripts/golden/sourceLabels.mjs` mounts Cindy's own component in
jsdom with Cindy's English strings, reads each label off the DOM and clicks it for its route. A label
that opens something is a Tab stop with the button role and lights to `foreground`; its `title` is
gpui's tooltip. A task's label reads the title the task list holds (`sessions::held_title`,
`getTitleById`). With no device list and no signed-in device before 13.0 every device label shows and
opens My devices; a teammate's mark is `BotAvatar`'s tint and initial, its profile coming with the
bots' chapter; remote tasks and the embedded view come with theirs. `orca_card.rs` is the Orca form
in place of the bubble: the labels set left (`Align::Start`) over a card up to 560 wide, whose header
(a Tab stop with the button role, lit to `cmd-palette-item-hover`, its corners rounded since gpui
clips to none) unfolds the message with the bubble's own record; the body is the display text as
`pre-wrap` rows (`draw::pre`) in the code face at the code size less 2px, each line the code size and
4px. Blink lays out no CR and no FF in `pre-wrap` (`logs/review/9.5q-r3-blink-controls.log`), so
`draw::laid` drops both first. Images, files and the goal badge are the bubble's. `hook_card.rs` is
the hook form, `HookTaskCard`: the device and plugin labels (`Over::Hook`: the card names its own
channel), the images and the files set left, then the card up to 560 wide. `src/hook_card.rs` holds
its words (`imLabel`, `hookTaskCardHeading`, the context's sections and what each counts), golden
`hook-card`: `scripts/golden/hookCard.mjs` mounts Cindy's component in jsdom, unfolds its context and
reads the DOM. The header is the channel's mark (Slack's and X's paths are Cindy's own) and who
spoke, a speaker's id as the tooltip. The words are `draw::pre` rows; clean stored text
(`contentFormat: 'user-text'`) past 14 rows keeps 10, the last ending in the ellipsis, under a
disclosure. A second disclosure with a chevron that turns over 150ms unfolds the attached context:
`Collapse` is a keyed transition of 200ms on the height the rows add up to and on the opacity, the
body unmounted once closed. A thread entry's `[author]` is semibold through shaped runs and a text
highlight. Both disclosures are Tab stops with the button role, lit to `text-secondary`, with a 2px
outline 2px off while the keyboard holds focus (an absolute bordered overlay). The fold's record keys
the context by `<key>-context`. The message actions come with 9.13. `flow.rs` breaks a line into rows
itself, since gpui's text holds no element: Blink's break opportunities (`breaks` is
`NextBreakablePosition`: a break after a run of spaces, the pair table
`resources/line-break-pairs.txt` for U+0021 to U+00FF, which `scripts/blink-line-breaks.js` reads off
Cindy's Electron, ICU's UAX 14 for the rest and for a hyphen before a letter past ASCII, with LB20a,
which ICU4X lacks) over the shaped glyphs' advances (`atoms`, read in x order, so right-to-left text
keeps its advances), a chip a fixed-width atom at U+FFFC, Blink's own stand-in; a word too long
breaks where it overflows (`overflow-wrap: anywhere`), and spaces, tabs and U+3000 at a wrap hang
unseen (`logs/review/9.5e-r1-blink-lines.log`, `-blink-spaces.log`). `flow/draw.rs` draws
`renderContentWithoutPastedText`'s nodes (`sent::parts`, each text piece apart) row by row, each row
a flex row of words, links and chips: no CR, a final line feed ends its line, a line that wraps fills
the bubble. A row's parts stand left to right; Blink's reordering of mixed directions across a chip
is not ported. The measure shapes with the window's text style, which no element above the text
changes. The fold counts the plain text's rows while the row is built, so Cindy's estimate and
remembered results are not ported; a folded message draws its first rows and `clamp` ends the last
with the ellipsis. A row with a chip grows as Blink's line box does: each chip's margin box (24px, a
quote island's 26px) centres half an x-height over the baseline
(`logs/review/9.5i-r1-blink-chip.log`). `chip.rs` is `InlineReferenceChip`'s inert face for the
legacy command, agent, folder and file chips (`FileTypeIcon` through `file_presentation`; lucide
0.468's `FileChartColumn` and `FileVideo` live in `lucide.rs`), sized by layout up to
`max-w-[min(240px,55vw)]`, with `Tip` after 300ms (`tip::mono` for a path). A label's white space
collapses to single spaces, as `truncate`'s `nowrap` draws it. `UserFileChip`'s click and menu come
with 9.13. `renderContent`'s stored ranges (`user_row::ranges`, `projectSentRanges` from where the
shown text starts in the content) cut the text first (`sent::tokens`): a slash range draws the
command chip, a pasted range its words with `tip::framed` (the text in a frame 320 wide that scrolls
past 256, hoverable), a reference `SentAgentReferenceChip`'s inert face. The fold's measure reads
those chips as their words (`inline_plain`). A `quotesEncoded` message is a list of `draw::Island`s:
each text island takes the ranges that lie in it (`sent::text_starts`, then `projectSentRanges` from
the island's start in the content), and a quote between two draws `QuoteChip` (`chip::quote`: `mx-1`,
no lift, the label through JavaScript's white space, the tip the quote in its marks over its source's
`chat_quotes::source_label`). A folded message reference draws that chip in line. `link_chip.rs` is
the face of `SessionLinkChip` and `ProjectLinkChip`, for a deep link in an unfolded message's text
and for its session, project and message references: the author's label, else the task's title
(`get`, the draft title as the unnamed label; a task of another device keeps `A session` until the
device link) or the folder's name; a message link is a quote chip, underlined and not lifted, of the
message's text (`resolveSessionMessageText` through `rows::anchor`, `sessionMessageDisplayText`). The
reads live with the mounted content (`Found`), asked once each after the frame that lacked them. A
chip that acts is underlined and lights to `surface-hover` under the pointer. A link that names no
task or folder stays its words. An image path is underlined and breaks as `word-break: break-all`
(`flow/break_all.rs`, Blink's `kBreakAllLineBreakClassTable`;
`logs/review/9.5l-r1-blink-breakall.log`). The message link's native `title` has no port: gpui holds
one tooltip. `link_chip/act.rs` is what those chips do, each a Tab stop with the button role: a
session link is `useSessionLinkTarget` (`resolveSessionRoute` read on the database worker: a Cindy
Make task goes to `/cc-agent`, an Orca worker to its lead, whose `?worker=` hint the router gains
with Orca's chapter; a chip's latest ask alone navigates, and only while the chip is mounted: a fold
ends its message's asks, and so does a stream render that draws the chip no more, at another place or
with another target (`Found.marks`); a modifier or the middle button asks for a task window, which
fails with `openLinkFailed` until that chapter; the message anchor's `searchJump` lands with 9.14), a
project link `sessions::focus::request`. Enter and Space act on key down with their modifiers; a
right click, the menu key or Shift+F10 opens `useLinkContextMenu`'s rows in the message's one menu
(`links::What`), Copy link last. An image path's click comes with 9.13. gpui snaps a line's height to
the device pixel, so a compact line is 22.5px for Cindy's 22.4. The unfolded messages live with the
mounted content, as the component's state does. `links.rs` is `UserMessageUrlLink` with
`useOpenWithMenu`: an unfolded message's text pieces are each read for web addresses (`linkify`),
underlined by text highlights, and a canvas after a piece lays a hit box over each stretch of a
link's glyphs, ordered by x since the shaper lists right-to-left text from the right
(`prepaint_as_root` at the laid-out text's positions). A link's boxes on every row share its Tab stop
(the first box), the menu and the record of boxes the menu key reads. A click is `activateMessageUrl`
(a modifier or the middle button opens the system browser, else the preference; the sidebar browser
comes with 17.0, so that open fails with Cindy's toast), Enter and Space activate on key down with
their modifiers, a right click, the menu key or Shift+F10 opens the menu (Ely `ContextMenu::manual`;
Ely's `Link` for `link-2`). gpui places an underline itself, so `underline-offset-2` has no port. In
tests `open_external` records the address and starts no program.

## A streaming message

`assistant_row.rs` reads the message's own `isStreaming`. `streamed.rs` is `MarkdownRenderer` for
such a message, kept with the mounted content by item key (`Streams`): the outer throttle
(`useStreamingThrottle` in the envelope) decides from its text whether a rich answer shows, which
draws nothing before 9.10 and unmounts the base; the base (`MarkdownRendererBase`) has its own
throttle on the same content, its fade state and its `Chunks`. `streamFade` is on unless reduced
motion or the switch says no; the state comes from `word_fade::cache::States` (a global, as the
module's cache) under `<task id>\0<clientId>`, asked for again only when the switch or the key
changed, and a message with no client id gets a state of its own. One timer per message waits for
the throttle that fires first (`arm`); `fire` runs each throttle due by then and draws the windows
again. `now` is `performance.now()`: milliseconds on the executor clock since `streamed::install`,
the app's start, so a stream's first text finds an interval already passed, as on a page loaded
long before. A message that streams no more drops this state and calls `releaseWordFadeState`
(`ended`); its tree is then the static one. Live rows come with 10.0: until then no stored row
streams, and the tests call `streamed::tree` on a mounted task.

`fade.rs` is `StreamFadeSpan` and `StreamFadeListItem` with the CSS they carry (`stream-word-in`
and `stream-marker-in`: `--motion-fast`, `--motion-ease-out`, filled both ways). Before a streaming
row is laid out `read` takes its tree's `stream-word` spans and its running markers (`Fading`, each
by its node's address) at `now`: a word's animation starts at its key's start on the timeline
(Cindy's delay is that start, rounded to the millisecond, from the span's mount), a marker's at the
start its item keeps (`wfBegin`). A word whose animation ended settles its key, as `animationend`
does, and draws whole; while an animation runs the window is asked for its next frame. The inline
reader notes each character's opacity as it writes it (`Swap::Fade`, `Inline.fades`): a space is
the span's that read it, and a pill a line's end moves keeps its own. A fade is
no face: gpui shapes each stretch of one colour alone, so a face per word would cost the kerning
between words and move the row's breaks. The text is laid out once, unseen, and
`inline/fading.rs` paints it over that box part by part, each part the whole text shaped in the
part's alpha and clipped to where the part's glyphs stand (`stretches`), so a right-to-left
word fades in its own place. An inline code's box and its pill draw at the
opacity of their place, a file chip's box as a whole (`Held.fade`), and a marker's colour takes
it as its alpha, as the keyframe's `transparent` does.
A word under a link the port draws as its words before 9.13 (the audio player) is on no timeline
and draws whole.

`streamed_chunks/markers.rs` is a list item's `::marker` animation as React and Blink keep it
(golden `stream-fade`; `logs/review/9.6j3c-blink-{marker,hold}.log`, probes in `.scratch/9.6j3c/`).
Each item React keeps has a record (`Marker`): the row by its chunk's start and fade state, the
block by its index and tag among the row's elements, each element under it by its tag's count. A
render writes what React writes: `data-stream-marker` only when React's own prop changed, so an
attribute `animationend` took off stays off while the prop stays; the delay React's style holds
for an item with no key; the delay the ref sets when the key changed and has not settled. An
animation lives while the attribute is on and the list draws markers (a task list's own class
leaves `list-style: none`, and Blink animates no marker it does not make); one that comes to life
starts then. A delay moves a running animation and leaves an ended one ended. `Chunks::advance`
is the frame before a render: an animation past its end has its `animationend`, which settles the
key the item rendered with and takes the attribute off, and with no key does neither. A running
marker's start is on its item (`wfBegin`), which the draw reads. `streamingMarkdownBlocks`' memo
needs no counterpart here either: a block it keeps has the same keys, so its items and words would
render to what they are.

Golden `stream-fade`: `scripts/golden/streamFade.mjs` bundles Cindy's `StreamingMarkdownChunk` for
a browser and `streamFadeRun.cjs` mounts it in an offscreen window of Cindy's Electron under
Cindy's own fade rules from `globals.css`, the animations run by Blink on the real clock: 19
written scenarios and 30 seeded ones (items with and without words, ordered, nested, quoted and
task items; words added, lines replaced). Each step is read 45ms clear of every animation's end
and holds the clock, each span's class, each item's attribute, animation and delay, and the
settled keys; the test replays the clock. Its times differ from run to run; its states do not.
`assistant_row::Live` lets a test draw a stored message as a streaming one.

## Selection

`select.rs` is the browser's selection over the stream, which the port keeps itself. Under
`.msg-markdown.select-text` every element computes `user-select: text`, a button and a chip too
(`.select-text *` comes after the `button` rule in `globals.css`;
`logs/review/9.6k1-blink-styles.log`, Cindy's `MarkdownRenderer` under Cindy's built CSS), so a
press anywhere in the scroller selects: the scroller takes it in the capture phase.

A drawn stretch of text is a run (`select::text` about the element that draws it): a canvas
under the text shapes it as its piece does, records its block's id, its first byte in the block's
text, its bounds, its row's line box (`select::line`, set by a row taller than its text), the
table cell about it (`select::scope`) and a cell per grapheme, left to right (`cells`: its bytes,
its x, and whether it reads right to left, since gpui's own index and x lookups walk glyphs as if
left to right; a letter reads by its own bidi class and a neutral as the glyphs about it stand, a
ligature's letters share its box in equal parts, and a mark shaped apart from its letter is no
place; `logs/review/9.6k1-r2-review-extra.log`), and fills the glyphs the selection holds in `--text-selection-bg`, the piece's
line high, at the opacity of a word on its way in. An inline block's id is its path and a code
block's its own; a code row's tab and a row with no text are runs with no glyph
(`select::space`): the tab one byte over its advance, the empty row one place.
`begin` empties the runs before the stream's rows draw. The two ends are a block's id and a byte,
so a wrap that moves a row keeps them; `cover` walks the runs in document order and lights what
lies between the ends, each end met once, an end in a space a wrap dropped at the next run.

A pointer's place (`at`) is Blink's (`logs/review/9.6k1-blink-edges.log`): in a row's line box the
nearest run's glyph, at the nearer of its two edges, so beside a row is the row's end on that
side; between rows the next row's start; under all the text its end. Inside a table's cell only
that cell's runs count (`logs/review/9.6k1-r1-review-blink.log`). A press sets both ends there, the drag
moves the focus on the window's mouse moves, past the scroller's edge. A press on lit text holds
the selection through the drag, as Blink drags the text, and a click there drops it when the
button lifts. A floating panel drops it (`unselect`, `PanelDragController`'s `removeAllRanges`).
A link chip's label and a handoff card's words are text too
(`logs/review/9.6k1b-blink-{card,grid,right,chip}.log`). Each is a flex item with a line of its
own (`select::label`, a canvas under the text its box draws and cuts, so a cut title's hidden
words select and its highlight stops at the box): in its line a pointer finds a glyph, over the
line the item's start and under it its end. A chip's icon and padding fall to the label's nearer
end. A card records its box, its circle, its lines and its arrow (`select::card`), and a pointer
level with it, or over it, reads them as Blink reads the flex row (`in_card`): left of the lines
the card's start, from the arrow on its end, between them the nearer of the two lines (the lower
one from half way down the gap) and in it the nearest item: the title, the badge (a run per
letter, since the port draws its tracking so, and one item), the history icon (a place with no
text), the words, the dot, the short id. A press on the card's bare face (not the circle, the
lines or the arrow) places no caret and keeps the selection. A press and a release on one card
still click it, as in Cindy.
A user message's bubble is `select-text` (`logs/review/9.6k1c-blink-user.log`, Cindy's
`renderContent`, `ChatBubble` and `QuoteChip` under Cindy's built CSS, the preload API a stub):
all of it computes `user-select: text` but the quote chip's two wrappers, whose label is text
again, since `.select-text *` follows `.select-none *` in `globals.css`; a press in that label
places a caret and a drag selects it. `flow/draw.rs` records each logical line as a block
(`<key>-l<n>`): its words by their bytes in the line, each chip's label (`Chip::selects`), a
line with no text as one place, each row its line box. `pre-wrap` keeps what a row's end
hides, so both are places past the row's last part, in a box that takes no room: the spaces a
wrap hangs (a run of their own, hit by glyph) and the line's break (`select::space`, a space wide
and the line high when selected; no caret stands after it, so right of a line is the line's
end). The last line has one too, the block's end, which Blink lights when the selection goes on
past the bubble. A quote chip's own box stays `select-none` (`select::quiet`): a press on it
places no caret and keeps the selection, but on its label and its icon (`select::loud`), which
are text again. A folded message's ellipsis is no text, and the row it cuts has neither box.
Not ported yet: the stream's other text, which `body`'s `user-select: text` makes selectable
(9.6k1d), the clicks and keys (9.6k2), copy (9.6k3, 9.6k4). A drag past the scroller's edge does not scroll it, where
Blink scrolls.
