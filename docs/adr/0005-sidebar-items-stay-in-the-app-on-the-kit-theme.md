# Sidebar items stay in the app, drawn from the kit theme

Supersedes [ADR 0003](0003-sidebar-items-are-built-in-the-app-from-ely-parts.md).

ADR 0003 built the sidebar's items in the app because Ely's `NavItem` could not show a game sprite
or a progress ring. Moving to GPUI Kit (#345), the kit's `sidebar` module was checked as the
replacement. What it covers and what it does not, at `gpui-kit` 0.10.0:

| Need (spec `docs/specs/app-shell.md`) | Kit `Sidebar` / `SidebarGroup` / `SidebarMenuItem` |
|---|---|
| A 20 px game sprite as the icon | No: `SidebarMenuItem::icon` takes an `Icon`, which draws an SVG only |
| An activity mark at the row's end | Yes, through `suffix` |
| The mark on the icon's corner in the rail (D5) | No: a collapsed item drops its label and suffix and centers its icon |
| A 56 px rail | No: the collapsed width is a fixed 48 px, set after the caller's style |
| Only the width animates; icons never move | No: a collapsed item re-lays out (`justify_center`, label removed) |
| Keyboard focus and Enter/Space on an item | No: the item takes no focus handle and draws no focus ring |
| Ids `nav-<section>` on the item | Partly: the menu numbers its items; a caller can render one with its own id |
| The app's `sidebar_scroll` handle, to reveal the active item and keep the offset | No: the column scrolls a `list` whose `ListState` is private |
| Group headings that do not fold, short in the rail | Partly: `SidebarGroup` hides its heading when collapsed |
| Tooltip in the rail | Yes, at the item's right |

So the column and `SidebarItem` (`ui/sidebar.rs`) stay the app's own, built from kit parts and the
kit theme only: `sidebar`, `sidebar_border`, `sidebar_foreground`, `sidebar_accent` (hover and
active fill) and `sidebar_accent_foreground` for the column and its items, `muted_foreground` for
the headings, `radius`, the kit `Tooltip`, `ThemeStyled::focus_ring_style` on a focus handle the
item keeps, `idleon_ui::widgets::bring_into_view`, and the kit's `transition` with
`ease_in_out_cubic` over 200 ms for the width (it finishes at once under reduced motion). Home's
icon is the kit's `IconName::House`.

The running mark (`ui/mark.rs`) stays a ring drawn in the app. The kit's `ProgressCircle` sizes its
stroke as 15% of its width (at most 5 px) and draws its track as the arc color at 20%, so at the
rail's 12 px its stroke is 1.8 px against D5's 2.5 px, and its track cannot be `input`. The ring is
drawn at D5's strokes in `primary` on `input`. The finished and failed dots are 8 px discs in
`Tone::Accent` and `Tone::Danger`: the kit's dot `Badge` pins itself over another element at its own
size and offset, while the rail mark sits on the app's disc in the row's background.

Considered: the kit `Sidebar` with `SidebarMenuItem`s and an SVG for Home only, dropping the sprites,
the 56 px rail, the corner marks and keyboard focus. That changes the approved design (D2, D5, the
fold flow) and the tab order, which is the owner's call, not a migration's. Once the kit's item
takes any element as its icon, keeps its layout when collapsed and takes focus, `SidebarItem` should
be replaced by it.
