# The window renders cached views that follow what they read

`AppState` was the window's only `Render` view. The header, sidebar, Home and every section were
`RenderOnce` builders under it, so any notify (a run's progress tick, a typed character, a hover)
rebuilt and laid out the whole window, and GPUI never reused anything (#337, #363).

`AppState` stays the root view and the one model, and keeps its cheap chrome: the history
shortcuts, the dialogs, the loader veil and the no-save banner. The rest is split into `Entity`
views in `ui/views.rs`, embedded with `.cached(style)`, so a frame replays a view's last output
unless that view was notified:

| View | Re-renders when |
|---|---|
| `Pane<HeaderReads>` | the fold, the history or the save status changes |
| `Pane<SidebarReads>` | the fold, the open place or any section's activity changes |
| `Pane<HomeReads>` | the search text or the section Resume offers changes |
| `Pane<StatusReads>` | the status line changes |
| `Page` (one per opened section) | any notify of `AppState`, or a run of its own section reports |

A `Pane` observes `AppState` and re-renders only when its `reads` function returns something
different from the last render's. A `Page` wraps the registry's unchanged
`fn(&mut AppState, &mut Window, &mut Context<AppState>)` through `Entity::update`, so section code
keeps its signatures. Its controls still `cx.notify()` `AppState`, and the open page follows every
notify. Views hold `WeakEntity<AppState>`, so the state still drops and flushes its session when
the window closes.

A run's events no longer notify `AppState`. `report_run` updates the activity and the status line,
then emits `RunReported { section }`. The panes check their keys against it, and only the owning
section's page re-renders. So a run in one section redraws the sidebar mark and the status line,
never Home or another section. A finished run schedules the session save that the notify used to
trigger.

The sidebar's cached box is laid out at the fold's width. Ely's `Sidebar` animates its own width
for 200 ms after a fold, which a fixed box would clip, so for that long the shell embeds the
sidebar uncached.

GPUI builds the AccessKit tree in prepaint and never replayed it for a cached view, so a replayed
view dropped out of the tree (`click_element: element "nav-home" does not support Click`, and the
same for a screen reader). The vendored GPUI in `vendor/gpui-mcp` now replays a reused range's
nodes, focus and action listeners (mastro993/gpui-mcp#4).

Considered:

- making each section's state its own entity, so GPUI's entity observation does the routing. That
  rewrites the 29 sections, which #348–#359 do one at a time anyway.
- one notify plus a "this came from a run" flag on `AppState`. Notifies coalesce within an effect
  cycle, so the flag can't tell a run's notify from a click's.
- views without `.cached`. An uncached child renders whenever its parent does, and the root
  renders every frame, so nothing would be saved.
