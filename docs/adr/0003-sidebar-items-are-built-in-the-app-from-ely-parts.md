# Sidebar items are built in the app from Ely parts

Ely's `NavItem` takes a vector `IconName` and a text count only, so it cannot show a game sprite or
a progress ring. The app renders its own `SidebarItem` (`ui/sidebar.rs`) with `NavItem`'s look
(Ely theme tokens for height, radius, active fill, hover and focus ring; tooltip when folded),
a manifest image for the icon, and Ely's `ProgressRing` and `DotBadge` for the activity mark. Ely's
`Sidebar`, `NavGroup`, `BackForwardNavigation` and `IconButton` are used as they are.

Ely's `ProgressRing` keeps its 48 px ring's 4 px stroke at any size and paints in `fg` on
`border`, so at the sidebar's 12–14 px it fills in. Until it takes a stroke and a tone, the running
mark is a small ring drawn in the app (`ui/mark.rs`) at D5's strokes, `accent` on `border_strong`;
the finished and failed marks are Ely's `DotBadge`.

Considered: a pull request to Ely adding an image icon and a trailing element slot to `NavItem`
(the right long-term home, but the pinned git revision would wait on an outside merge; once it
lands, `SidebarItem` should be replaced by `NavItem`).
