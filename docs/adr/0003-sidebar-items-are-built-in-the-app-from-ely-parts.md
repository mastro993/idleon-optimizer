# Sidebar items are built in the app from Ely parts

Ely's `NavItem` takes a vector `IconName` and a text count only, so it cannot show a game sprite or
a progress ring. The app renders its own `SidebarItem` (`ui/sidebar.rs`) with `NavItem`'s look
(Ely theme tokens for height, radius, active indicator, hover and focus ring; tooltip when folded),
a manifest image for the icon, and Ely's `ProgressRing` and `DotBadge` for the activity mark. Ely's
`Sidebar`, `NavGroup`, `BackForwardNavigation` and `IconButton` are used as they are.

Considered: a pull request to Ely adding an image icon and a trailing element slot to `NavItem`
(the right long-term home, but the pinned git revision would wait on an outside merge; once it
lands, `SidebarItem` should be replaced by `NavItem`).
