# Tables, scrolling and charts

How a migrated screen shows rows, scrolls and plots with GPUI Kit, and how it stays fast doing it.
It covers Ely's `tables::Table`, `layout::ScrollArea`, `charts::LineChart` with `Series`, and
`lists::List`/`ListItem`. Tracking issue: [#335](https://github.com/mastro993/idleon-optimizer/issues/335).

## Tables

Ely's `Table::new([headers]).row([..])` builds every row on every render. The kit has two tables,
and the choice is not a matter of taste:

| Rows | Use | Why |
|---|---|---|
| Fixed and small: bounded, at most about 50 (a stat breakdown, a currency summary, a plan's steps) | `table::Table` | Stateless and composable; building 50 rows a frame is cheap |
| Unbounded or larger (results, ranked candidates, logs, history, per-item lists) | `table::DataTable` with a delegate | Virtualized: a frame builds only the rows in view |

When in doubt, ask whether the row count depends on the save, a run or user input. If it does, it
is unbounded.

A `DataTable` scrolls itself, so it needs a pane of its own (see [Scrolling](#scrolling)). Where a
page already scrolls as a whole and its tables sit in it, as Royal Guardian's panes do, the tables
stay `Table`s: their rows are bounded by a game catalog (the 69 Armory shelves, a world's outposts
or nodes) or by the run's own caps, and the engine results behind them are cached in section state
so a frame only formats them.

### Small tables: `Table`

```rust
// Before (Ely)
let mut table = Table::new(["Currency", "Start", "At the horizon", "Collected"]);
for row in &rows {
    table = table.row([row.name.clone(), row.start.clone(), row.end.clone(), row.collected.clone()]);
}

// After (kit)
use gpui_kit::component::table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow};

Table::new()
    .child(TableHeader::new().child(
        TableRow::new()
            .child(TableHead::new().child("Currency"))
            .child(TableHead::new().text_right().child("Start"))
            .child(TableHead::new().text_right().child("At the horizon"))
            .child(TableHead::new().text_right().child("Collected")),
    ))
    .child(TableBody::new().children(rows.iter().map(|row| {
        TableRow::new()
            .child(TableCell::new().child(div().id(row.id.clone()).child(row.name.clone())))
            .child(TableCell::new().text_right().child(row.start.clone()))
            .child(TableCell::new().text_right().child(row.end.clone()))
            .child(TableCell::new().text_right().child(row.collected.clone()))
    })))
```

Number columns are right-aligned (`text_right()` on the head and its cells). An element id the
harness selects stays on the same element: wrap the cell's content in a `div().id(..)`, as above.

Give each `Table` an id through a wrapper (`div().id("<id>-table").child(Table::new()…)`) when
another table sits under the same nearest element id, as in a pane of cards (a `Card` carries no
id). The kit derives a table's accessibility node from that id path, so two such tables collide and
a debug build panics with `Duplicate a11y node id`.

Give each `TableRow` a short id of its own (`TableRow::new().id(("forecast-row", ix))`). The kit's
default row id spells out the type path of the table, its body and the row, and from inside a
scroll area with a long id it outgrows the bridge's 256-byte limit. The bridge then drops the row
and everything in it from the semantic tree, so the harness cannot find ids inside that row, even
though the row is drawn.

### Large tables: `DataTable` and `RowsDelegate`

A `DataTable` draws a `TableState<D>` entity whose delegate `D: TableDelegate` answers
`rows_count` and renders one cell at a time through `render_td`. The table asks only for the rows
in view (plus the first row, which the row list lays out to measure the row height), so a frame
costs a screenful of cells whether there are 50 rows or 5,000.

For the common case, a `Vec<T>` of rows plus column definitions, use
`idleon_ui::widgets::RowsDelegate<T>` instead of writing a delegate:

- `RowColumn::text(key, name, |row| ..)` is a left-aligned text column;
- `RowColumn::number(key, name, |row| ..)` is right-aligned, in the theme's mono face at
  `TEXT_MONO` (`design.md` § Type) with tabular figures (`tnum`), so digits line up down the column
  even while the mono face falls back to the system font;
- `.column(|c| c.width(px(140.)))` adjusts the kit `Column` (width, fixed side, resizing);
- the same text feeds the cell's accessibility label and `cell_text` (export).

The pattern has three rules:

1. **The `TableState` lives in section state**, built once in the section's `State::new`
   (`docs/section-state.md`), never in `render`.
2. **Rows are built where the data changes**, not in `render`: when a run finishes, the save
   reloads or an input changes, the section turns its domain data into row structs and hands them
   to the delegate with `set_rows`. Sorting and filtering happen there too.
3. **`render` only names the table**: `DataTable::new(&self.table)`. The delegate's cell closures
   read a row it already holds; keep them to field access and number formatting.

```rust
// Before (Ely): every frame formats every candidate.
fn candidates(result: &Plan) -> AnyElement {
    let mut table = Table::new(["Candidate", "Gain", "Cost"]);
    for candidate in &result.candidates {
        table = table.row([candidate.name.clone(), format::rate(candidate.gain), format::number(candidate.cost)]);
    }
    table.into_any_element()
}

// After (kit): the rows are built once per result and the table draws the visible ones.
use gpui_kit::component::table::{DataTable, TableState};
use idleon_ui::widgets::{RowColumn, RowsDelegate};

pub(crate) struct State {
    pub(crate) candidates: Entity<TableState<RowsDelegate<CandidateRow>>>,
    …
}

pub(crate) struct CandidateRow { name: SharedString, gain: SharedString, cost: SharedString }

impl State {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let delegate = RowsDelegate::new(vec![
            RowColumn::text("name", "Candidate", |row: &CandidateRow| row.name.clone()),
            RowColumn::number("gain", "Gain", |row: &CandidateRow| row.gain.clone()),
            RowColumn::number("cost", "Cost", |row: &CandidateRow| row.cost.clone()),
        ]);
        Self {
            candidates: cx.new(|cx| TableState::new(delegate, window, cx)),
            …
        }
    }

    /// Called when the run reports its result, not from `render`.
    fn show(&mut self, plan: &Plan, cx: &mut Context<AppState>) {
        let rows = plan.candidates.iter().map(CandidateRow::from).collect();
        self.candidates.update(cx, |table, cx| {
            table.delegate_mut().set_rows(rows);
            cx.notify();
        });
    }
}

// In render:
div().id("candidates").h(px(360.)).child(DataTable::new(&state.candidates))
```

The table fills its parent, so give the parent a height (or `flex_1().min_h_0()` in a column).
When rows need more than text (selection, sorting, a badge or a button in a cell, a per-row element
id), implement `TableDelegate` for the section's own type; the three rules still hold. The kit
gives each drawn cell the id `table-cell:<row>:<col>` inside the table; rows out of view are not
drawn, so the harness can only select what is on screen.

`crates/ui/src/widgets/table.rs` has the test that keeps this honest:
`a_long_table_builds_only_the_rows_in_view` draws a 5,000-row table in a 400 px window, checks that
fewer than 50 rows are built, scrolls to row 4,000 and checks again.

### Lists

Ely's `lists::List`/`ListItem` had one use, the spelunking event log. A short fixed list maps to kit
`list::ListItem` rows in a plain column. An unbounded list of uniform rows uses `v_virtual_list`
(below) or a `DataTable`: the event log is a `DataTable` (`sections/spelunking/events.rs`) whose
first cell keeps the `spelunk-run-event-<n>` id.

A long list whose rows differ in height, or change height when one expands, uses GPUI's own
`list` over a `ListState` kept in section state: it measures rows as they scroll into view, so it
needs no sizes up front. Give it a fixed-height parent and `vertical_scrollbar(&list_state)`, call
`reset(count)` when the rows change and `remeasure_items(ix..ix + 1)` when one expands. The Ladle
reset forecast (`sections/ladle/forecast.rs`) is one: up to 365 expandable resets.

Every kit `Table` draws under the same element id, so two tables with no element id between them
collide (a debug build panics on the duplicate accessibility node). Give each table a `div().id(..)`
of its own.

## Scrolling

| Before (Ely) | After (kit) |
|---|---|
| `ScrollArea::new(id).vertical().child(body)` | `div().overflow_y_scrollbar().id(id).child(body)` |
| `ScrollArea::new(id).child(tabs)` (sideways) | `div().overflow_x_scrollbar().id(id).child(tabs)` |
| `ScrollArea::new(id)` scrolling both ways | `div().overflow_scrollbar().id(id).child(body)` |
| A long list of same-height rows | `v_virtual_list(view, id, item_sizes, \|view, range, window, cx\| ..)` |

`overflow_*_scrollbar` comes from `gpui_kit::component::scroll::ScrollableElement`. Set the id on
the `Scrollable` it returns, after the call: that id names the scroll area's outer element (as
`ScrollArea::new(id)` did, so the harness still finds it) and keys its scroll position. An id set on
the `div` before the call is replaced. Without an id the scroll position is keyed by the call site,
so a helper that builds several scroll areas, like `ui::layout`'s, must pass one.
`v_virtual_list` (`gpui_kit::component::v_virtual_list`) builds only the items in the visible
range; a `DataTable` already does this for its rows, so never nest one inside another scroll area.

**Scroll position across navigation.** Today a `ScrollArea` keeps its offset in GPUI element state
keyed by its id, and GPUI drops element state that a frame does not draw. So a section scrolls back
to the top when the user leaves it and comes back, and every scroll position resets on restart. The
one exception is the sidebar, whose `ScrollHandle` lives in `AppState` (`sidebar_scroll`). The
kit's `Scrollable` keeps its handle in keyed element state too, so migrating keeps today's behavior
with no extra work. Where a position should survive navigation, keep a `ScrollHandle` in the
section's state and drive the area with it:

```rust
div()
    .id("royal-pane")
    .track_scroll(&state.royal.scroll)
    .overflow_y_scroll()
    .vertical_scrollbar(&state.royal.scroll)
    .child(body)
```

A `DataTable` already does this: its scroll handle lives in its `TableState`, which lives in section
state, so a large table keeps its position across navigation.

## Charts

Ely's `LineChart::new(id, labels).series(Series::new(name, values))` maps to the kit's
`chart::LineChart`, which plots a list of points through `x` and `y` accessors:

```rust
// Before (Ely)
LineChart::new("projection-chart", labels)
    .series(Series::new("EXP/hr", rates))
    .format(format::rate)
    .h(px(160.0))

// After (kit)
use gpui_kit::component::chart::LineChart;

div().h(px(160.0)).child(
    LineChart::new(samples.iter().copied())
        .id("projection-chart")
        .name("EXP/hr")
        .x(|sample: &Sample| format::hours(sample.hours))
        .y(|sample: &Sample| sample.rate)
        .y_tick_format(format::rate)
        .stroke(cx.theme().chart_1),
)
```

- The chart fills its parent: set the height on the wrapper `div`.
- Series colors come from the theme's chart tokens: `cx.theme().chart_1` to `chart_5`, with
  `chart_grid` for grid lines and `chart_bullish`/`chart_bearish` for up/down. Never a literal color.
- The points are the section's cached data; the chart reads them through the accessors, so do not
  copy them into label and value vectors each frame.
- The kit `LineChart` draws one series. The app's three Ely line charts each draw one. A chart with
  several lines, gaps or dashes is built from the kit's `plot` primitives (scales, shapes, axes:
  `gpui_kit::component::plot`), as the spelunking depth chart (`sections/spelunking/chart.rs`)
  draws its own canvas today.
