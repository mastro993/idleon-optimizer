# The sidebar is the primary navigation; the dashboard stays as Home

Every move between sections went back through the dashboard card grid (`plan.md` §3, "Dashboard
first"). A persistent sidebar grouped by world now opens any section from any other, with page-level
back/forward history in the header. The card grid stays as Home, the first sidebar entry, so its
search, descriptions and first-launch landing are kept, and the harness still opens sections
through `card-<id>`.

Considered: dropping the grid and reopening the last section at launch (turned down by the owner:
it removes the searchable, described overview and the first-launch landing); history that also
records tab switches inside a section (turned down: noisy, and it would make every section's tab
strip report to the shell, against `section-state.md`).

Consequences: Escape no longer leaves a section, and the "Back to dashboard" button is gone; back
and forward use Cmd/Ctrl+[ and Cmd/Ctrl+], plus Cmd/Ctrl+Left and Right. The five hidden tools get a
sidebar entry under Tools, but still no dashboard card.
