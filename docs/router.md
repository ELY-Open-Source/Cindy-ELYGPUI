# Router, task rows and search

The router and the sidebar's task rows, selection and search, moved from `AGENTS.md`.

`src/router.rs` is `router.tsx`'s hash routes for chapter 8: one location in the `Router` global,
starting at `/`: its pathname, which the routes match, and the search a link carries for the page it
opens. `route` is React Router 7.18's `matchRoutes`: `decodePath` decodes each segment first (a
pathname that does not decode stays as written) and a param's `%2F` comes back a slash, so
`active_session` is the task's own id. The last chat view is `useTrackLastView`'s `matchPath` on the
pathname as it stands (`matched`), so an id with escapes is kept escaped, as Cindy keeps it. `/`
goes to `/cc-agent` (`MainEntryRedirect`; the remembered entry needs 13.0's owner-scoped storage);
`/cc-agent` waits for the task list, then opens the last chat view (`lastViewStore`'s chat slot,
kept by `navigate` as `useTrackLastView` does), the first task, or `/cc-agent/new`. The chat pane
shows its loading line meanwhile. `sessions/open.rs` is `handleSessionClick`: a plain click on a
row, list row or card opens `/cc-agent/<id>` unless it is shown; a Shift, Mod or Ctrl click waits
for 8.6d's selection. The active task (`router::active_session`) draws as Cindy's: its pill and ring
(an absolute bordered overlay stands in for the inset shadow), foreground and tints. The task menu
wraps a renamed row too, so a press that commits a rename lands its click; the field stops right
presses. `sessions/keyboard.rs` hosts that menu (Ely `ContextMenu::manual`) and makes each row, list
row and card a Tab stop: Enter or Space on the row itself opens it; the menu key (key up on Windows)
or Shift+F10 (not macOS) opens the menu at the focused row's or More's centre, as Chromium's
`ShowNonLocatedContextMenu` does. Prepaint probes record those visible bounds. `select.rs` is the
bulk selection of `handleSessionClick`: the row probes also list the rows painted (the `Drawn`
global, pinned card slots marked with `rowMajorRank`), which stands in for
`getVisibleSidebarSessionIds`; a probe at the list's end prunes the selection, as the
MutationObserver does. The bulk bar has Clear; Archive and Delete come with 18.0. `reveal.rs` is
`scrollIntoNearestView`: a row that turns active, or mounts active, is scrolled to CSSOM's `nearest`
on the list's `ScrollHandle`, along Chromium 148's desktop programmatic curve (`cubic-
bezier(0.4,0,0,1)`, `min(sqrt(|delta|), 90) / 60` s in device pixels); reduced motion jumps, and a
scroll by the user or a reveal that needs no move ends it. The active record is keyed by look (text,
list, card), since Cindy mounts a new component per look. `search.rs` is the conversation search:
two `useConversationSearch` instances (`Which`), the inline one `conversationSearchContext` shares
and the rail popover's, each with its own Ely `TextInput`, filters and run, one sort on `Search` for
both; and the top nav's Search row (`SidebarInlineSearch`) at the top of the list's scroller. Focus
goes straight to the field, which stands for `forceOpen`; the placeholder is drawn over the field in
`text-placeholder`, since Ely draws its own in `fg_subtle`. A press that does not hit the row
(gpui's hit test, so a clipped row misses) clears a running query; hover ignores key presses.
`search/options.rs` is `SearchFilterMenu` on Ely's `Menu` (`menu_under`) in the open row: sort
(saved under `cc-agent.search.sortBy`), status, projects, agent and last activity, reset and the
reminder dot. Focus inside the menu stands for `menuOpen`; a press past the open menu clears the
query once the press is done, unless a row took it (Ely chooses on mouse down). `search/projects.rs`
is the Projects filter: `allKnownProjects` from a second read of every status (Orca workers, pins
and hidden projects left out), `nextProjectSelection`, and the selection reconciled after each read.
Its tasks' ids scope the search, so a choice that adds no task keeps the page; the list's scroll
keys on the selection. `group.rs` sorts projects by their latest task or `lastUsedAt`. Ely's rows
have no value slot, so a project's task count joins its label. `search/run.rs` is
`useConversationSearch`'s effect: a change of the trimmed query or an option starts over (dropped
tasks clear the timers); the keyword pass at 250ms and the hybrid pass at 900ms run
`searchConversations` on the database worker, only the latest publishes, the hybrid page wins once
it lands, and an empty page waits for both. `fanout` is the renderer's
`mergeConversationSearchFanout` (Cindy Make rows dropped, `max(userSendAt, updatedAt)`).
`search/results.rs` is `SearchResultsBody` with its rows, drawn while a query runs; the list stays
built under `display: none`, so gpui skips its prepaint and its rows leave the selection, as Cindy's
hidden rows do. Its hitbox joins the search surface. `marks.rs` draws title and keyword marks as
text highlights, which have no padding or radius. A click on a result or hit is `handleSelect`: its
task opens and the query stays; the hit's `searchJump` lands with the chat view (9.14). Results,
hits and More are Tab stops with the button role; gpui clicks them on Enter or Space key up.
`search/scroll.rs` is the sidebar's layout effect: the list's offset is kept while it shows, each
search starts at the top, and a clear puts the offset back once the pointer lifts. `search/stick.rs`
is `pinSearch`: while a query runs the row's prepaint offsets it to the scroller's top (gpui has no
`position: sticky`), and the content after it paints and hits under a content mask that starts at
the row's bottom, as the clip-path effect clips. `search/lock.rs` is `requestConversationSearch`
with the lock it brings. The project menu's Search and the dialogues' (`sessions/dialogue.rs`,
`SessionGroupNode`'s header: a Tab stop, its menu on a right click, the menu key or Shift+F10 under
its left edge) lock the inline search, and the rail's while its view shows (`isCollapsed`), to a
project (`[key]` selected, its own ids while none is known) or a task group (sorted ids, counted as
a filter). The field takes focus; the inline row shows the chip (`search/chip.rs`), whose X is
`clearLock`, as the rail tile's click is. The options disable the project rows and Reset keeps the
locked project. A project hidden or not known lets the lock go. While a menu's search opens with no
query, `scroll.rs` freezes the list at the top until a press outside the search lifts (a press past
an open menu counts unless the menu chose) or a Tab stop outside it takes focus (`focusin`; the
root's focus is not a Tab stop, as `body` is not).
