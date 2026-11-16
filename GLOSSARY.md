# Glossary

Terms the app shell and the section code share. Game terms (Prisma, Opal, Cglunko, …) keep the
names the game gives them and are not listed here.

- **Section**: one calculator page in the registry (`sections::Section`), with its own state, tab
  strip and runs. Code says `section`; the UI may say "calculator". Avoid "page" for a section in
  code, and "tool" except for the Tools group.
- **Group**: the world a section belongs to (`sections::Group`: General, W2–W7, Tools). The UI
  says "World 2"; code says `Group::W2`. Avoid "category".
- **Tools**: the sections with no dashboard card (`dashboard: false`). The sidebar lists them in
  its own Tools group; the dashboard still hides them.
- **Home**: the dashboard card grid, reached from the top of the sidebar (`Route::Dashboard`).
- **Shell**: the window chrome around a section: header and sidebar (`ui/shell.rs`).
  A section never renders shell parts itself.
- **Header**: the bar across the top of the window: sidebar toggle, history arrows, save status,
  Import and Export.
- **Sidebar**: the column of sections down the left, grouped by group. It is either **expanded**
  (icon and title) or folded to the **rail** (icon only, title in a tooltip). Avoid "drawer",
  "nav bar".
- **Place**: where the window is: Home, a section, or a missing id. One entry in the history.
- **History**: the places opened in this window, oldest first, with a current position; the back
  and forward arrows move through it. Tabs inside a section are not places.
- **Run**: one background job of a section (a simulation, optimizer or search) on its
  `TaskRunner`. A section may own several runners. Avoid "task" and "job" in UI text.
- **Activity**: what the sidebar shows about a section's runs: **running** (with its progress),
  **finished** (a run stored a result while you were elsewhere, until you open the section) or
  **failed** (a run panicked while you were elsewhere, until you open the section).
- **Save status**: what the header says about the imported save (`SaveStatus`: No save, Save
  imported, Save imported with warnings, Import failed).
