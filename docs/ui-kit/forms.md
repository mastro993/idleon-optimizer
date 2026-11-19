# Buttons and forms

How each Ely button and form control mapped to GPUI Kit, and **the one pattern every section
uses to own a kit control's state**. Written for #341; every section issue (#348–#359) and the
shell issues followed it. Ely is gone since #361: the mapping stays as the record of what each
kit control replaced, and the pattern is how a new control is built.

## Mapping

Paths are under `gpui_kit::component` (the kit) or `idleon_ui::widgets` (our gaps). The count is
how many app files used the Ely item when this page was written.

| Ely | Uses | Kit or `idleon_ui` | Notes |
|---|---|---|---|
| `buttons::Button` | 88 | `button::Button` | `Button::new(id).label(text)`; the id stays the Ely id. `on_click` has the same signature. |
| `ButtonVariant` | 30 | `button::ButtonVariants` | `Primary` → `.primary()`, `Secondary` → no call (the kit default: surface and border), `Outline` → `.outline()`, `Ghost` → `.ghost()`, `Subtle` → `.secondary()`, `Success` → `.success()`, `Danger` → `.danger()`, `Link` → `.link()`. |
| `IconButton` | 5 | `button::Button` | `Button::new(id).icon(IconName::…).ghost().tooltip(label)`. Lucide names beyond the kit's default set come from `gpui_kit::assets::IconName`. |
| `CopyButton` | 3 | `clipboard::Clipboard` | `Clipboard::new(id).value(text)`; `on_copied` replaces any status line the section set itself. |
| `SegmentedControl` | 11 | `button::ToggleGroup` | `.segmented().single()` with one `Toggle::new(id).label(..).checked(current == id)` per segment. `on_click` hands the checked flags; a click on the checked segment clears them all, so treat an all-false vector as "keep the current value". |
| `ToggleGroup`/`ToggleItem` | 4 | `button::ToggleGroup`/`Toggle` | Same as above without `.segmented()`. |
| `forms::Select` | 57 | `select::Select` over `widgets::ChoiceSelect` | Stateful: see [the pattern](#state-ownership). `Select::new(state).id(ely_id)`. |
| `Choice` | 58 | `widgets::Choice<V>` | `Choice::new(value, label)`, with a typed value (`usize`, an enum, `Option<_>`) instead of Ely's string. Ely's `icon` and `avatar` were unused; a `note` joins the label (`Name · note`), as in Spelunking's character and cave pickers. |
| `NumberInput` | 28 | `input::NumberInput` over `widgets::number_state` | Stateful: see [the pattern](#state-ownership). Ely's `range`/`step` move to `number_state(min, max, step, …)`; `precision` moves to `show_number`. |
| `Checkbox` | 28 | `checkbox::Checkbox` | Stateless, as in Ely: `Checkbox::new(id).label(..).checked(value).on_click(\|checked, window, cx\| …)`. |
| `Stepper` (−/+) | 20 | `input::NumberInput` | **Not** the kit's wizard `stepper::Stepper`. The kit `NumberInput` already steps with its buttons and Up/Down, so one control covers both Ely's `NumberInput` and `Stepper` (the #337 baseline found two sets of arrows on one field). It can now also be typed in. |
| `TextInput` | 13 | `input::InputState` | `InputState::new(window, cx).placeholder(..)`; Ely's `.label(..)` becomes `aria_label` on the render line. `multi_line(min, max)` → `TextareaState::new(window, cx).auto_grow(min, max)`. `text()` → `value()`, `set_text(t, cx)` → `set_value(t, window, cx)`, `focus()` → `Focusable::focus_handle(cx)`. |
| `Input` | 8 | `input::Input` / `input::Textarea` | `Input::new(&state).aria_label(label)`; `.size(ControlSize::Sm)` → `.small()` (`Sizable`). |
| `InputEvent` | 6 | `input::InputEvent` | `Changed` → `Change`. The kit also emits `PressEnter`, `Focus` and `Blur`. |
| `SearchInput` | 9 | `widgets::search_input` | A kit `Input` with a magnifier and a clear button. Ely's history popup was unused. |
| `Slider` | 3 | `slider::Slider` over `SliderState` | Stateful like `NumberInput`: `SliderEvent::Change` while dragging, `Release` once. |
| `DropZone` | 1 | `widgets::DropZone` | The kit has file chips (`attachment`) but no drop target. Same id, `kinds`, `hint` and `on_drop`, one file at a time; Browse is `(id, "browse")`. |
| `Run` | 1 | `Rc<dyn Fn(&mut Window, &mut App)>` | Ely's name for a click handler; spell the type out or alias it locally. |
| `overlays::Dialog` | 2 | `dialog::Dialog` | Opened through the kit `Root` with `window.open_dialog(cx, \|dialog, window, cx\| …)` (`WindowExt`), not rendered inline. The state method that opens it keeps an `open` flag; the dialog's `on_close` (Escape, backdrop, close button) and the state's own close both clear it, and only the state's close calls `window.close_dialog`. Ids go on a `v_flex().id(..)` body, the actions in a `DialogFooter`. See `open_import` in `state/documents.rs`. |

### Disabled and secondary buttons

The #337 baseline found "Reset" and "Reset all edits" nearly invisible. Use the kit's own
`.disabled(true)` and variants; never lower a control's opacity or recolor it by hand. The kit paints
a disabled label in the theme's `muted.foreground` at 50% opacity: that is 2.6:1 on the background
with today's theme and 2.5:1 (dark) / 2.3:1 (light) with the Console theme. #360 has to raise it to
at least 3:1 through the theme, so every disabled control gets legible at once.

## State ownership

Ely's form controls were builders: the section passed its value every frame and got a callback.
The kit's `Select`, `NumberInput`, `Input` and `Slider` render an **entity** that holds the
control's own state. The rule:

1. **The section's plain value stays the source of truth.** It is the field `config()` writes and
   `apply_config` restores (`docs/section-state.md`), exactly as with Ely. The control never owns a
   persisted value.
2. **The section `State` owns the control entities**, built once in `State::new(window, cx)` and kept
   in private fields next to the value they show. Never build one in render: a rebuilt entity loses
   focus and the caret, and the #337 baseline counted about 25 inputs re-rendering per frame in
   Spelunking. An app-wide field that no single section owns stays on `AppState` the same way.
3. **Edits flow control → value** through a subscription made in `State::new` and kept in
   `_subscriptions`. It parses the control's event and calls the section's setter, which clamps,
   stores and `cx.notify()`s. Nothing else writes the value from the control.
4. **Values flow value → control in render**, through `ChoiceSelect::show` and `show_number`. They
   write only when the control shows something else, skip a field the user is typing in, and emit
   no change event, so there is no feedback loop. This is what lets `on_import` and `apply_config`
   stay window-free: they set the value, and the next frame brings the control in line.
5. **The element id stays the Ely id.** Put it on the control when the kit builder has `.id(..)`
   (`Select`, `Input`, `Button`, `Checkbox`, `Toggle`); otherwise wrap the control in
   `div().id(ely_id)`, as `NumberInput` needs. `script/mcp-*.mjs` and `sections/tests.rs` click and
   read those ids. In the app, `ui::number_field(id, &state, label)` is that wrapper for a small
   `NumberInput`: a press on it focuses the field, so the bridge can click it. Ely's stepper ids
   (`<id>-up`, `<id>-down`, `<id>-more`, `<id>-less`) have no kit equivalent, because the kit's step
   buttons all share the local ids `increment` and `decrement`, which the bridge drops as duplicates.
   A harness clicks the field and presses `up` or `down` instead.

### Worked example

The Opal Optimizer's target `Select` and one villager's opal `NumberInput`, converted. The section
itself (`sections/opal/state.rs`) follows it with a few more controls; this trimmed version compiles
and runs as
[`crates/ui/tests/form_state.rs`](../../crates/ui/tests/form_state.rs), whose regions this page
quotes verbatim (`the_doc_quotes_this_example` fails when they drift). `AppState` there stands in for
the app's window state.

The state: the persisted `target` and `manual` values, the controls, and the subscriptions that
carry edits back.

```rust
/// The section's state: the values it persists, and the kit controls that
/// show them. The controls are built once, here, never in render.
pub(crate) struct State {
    /// Persisted. `None` targets every villager's total EXP.
    pub(crate) target: Option<usize>,
    /// Persisted. The opals typed in for each villager.
    pub(crate) manual: [f64; VILLAGERS],
    /// How many villagers the save unlocked; set by `on_import`.
    pub(crate) unlocked: usize,
    target_select: ChoiceSelect<Option<usize>>,
    manual_fields: [Entity<InputState>; VILLAGERS],
    _subscriptions: Vec<Subscription>,
}

impl State {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let target_select = ChoiceSelect::new(target_choices(0), Some(&None), window, cx);
        let manual_fields = std::array::from_fn(|_| number_state(0.0, MAX_OPALS, 1.0, window, cx));
        // Edits flow control -> section value, through the section's setters.
        let mut subscriptions = vec![cx.subscribe(
            target_select.state(),
            |state, _, event: &SelectEvent<Vec<Choice<Option<usize>>>>, cx| {
                let SelectEvent::Confirm(target) = event;
                state.opal.set_target(target.flatten(), cx);
            },
        )];
        for (villager, field) in manual_fields.iter().enumerate() {
            subscriptions.push(cx.subscribe(field, move |state, field, event, cx| {
                if matches!(event, InputEvent::Change)
                    && let Some(opals) = number_value(field.read(cx))
                {
                    state.opal.set_manual(villager, opals, cx);
                }
            }));
        }
        Self {
            target: None,
            manual: [0.0; VILLAGERS],
            unlocked: 0,
            target_select,
            manual_fields,
            _subscriptions: subscriptions,
        }
    }

    pub(crate) fn set_target(&mut self, target: Option<usize>, cx: &mut Context<AppState>) {
        self.target = target;
        cx.notify();
    }

    pub(crate) fn set_manual(&mut self, villager: usize, opals: f64, cx: &mut Context<AppState>) {
        self.manual[villager] = opals.clamp(0.0, MAX_OPALS);
        cx.notify();
    }

    /// `apply_config` writes the persisted values only: it has no `Window`,
    /// and the controls catch up on the next render.
    pub(crate) fn apply_config(&mut self, config: Config, cx: &mut Context<AppState>) {
        self.target = config.target;
        self.manual = config.manual;
        cx.notify();
    }
}

/// The target list: everyone, then each unlocked villager.
fn target_choices(unlocked: usize) -> Vec<Choice<Option<usize>>> {
    std::iter::once(Choice::new(None, "All villagers (total EXP)"))
        .chain((0..unlocked).map(|villager| Choice::new(Some(villager), NAMES[villager])))
        .collect()
}
```

The render side: the section's render passes its values on, then draws the controls.

```rust
/// Values flow section value -> control here: `show`/`show_number` write
/// only when the control shows something else, and never emit an edit.
fn target_select(state: &AppState, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let opal = &state.opal;
    let choices = target_choices(opal.unlocked);
    opal.target_select
        .show(choices, Some(&opal.target), window, cx);
    Select::new(opal.target_select.state())
        .id("opal-target")
        .accessibility_label("Target")
        .small()
}

fn manual_input(
    state: &AppState,
    villager: usize,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let field = &state.opal.manual_fields[villager];
    show_number(field, state.opal.manual[villager], 0, window, cx);
    // `NumberInput` has no `id`: a wrapper carries the control's id.
    div().id(format!("opal-manual-{villager}")).child(
        NumberInput::new(field)
            .accessibility_label(format!("{} opals", NAMES[villager]))
            .small(),
    )
}
```

What changes against the Ely section:

- `State` gains `target_select`, `manual_fields` and the subscriptions; `target`, `manual`,
  `config()`, `apply_config` and `on_import` keep their shape, so the project JSON is unchanged.
- The Ely `on_change` closures in render become the two subscriptions in `new`.
- Render builds the target list as before (it is small), and `show` pushes it only when it changed.
  A list that is large or costly to build is cached on `State` and rebuilt on import instead.
- A bound that depends on the save, like Ely's `range(0.0, MAX_OPALS.max(opal.total_opals()))`,
  is enforced by the setter's clamp; `number_state`'s `min`/`max` hold only the static range.

## App-wide text inputs

The text fields on `AppState` moved to the kit in #341, because they live in shared files that the
section issues must not edit at the same time:

| Field | Kit state | Render line |
|---|---|---|
| `search` | `InputState` | `ui/dashboard.rs`: `search_input(..)` in `dashboard-search` |
| `monument_search` | `InputState` | `sections/monument/justice.rs`: `search_input(..)` |
| `stats_explorer_search` | `InputState` | `sections/stats_all/explorer.rs`: `search_input(..)` in `stats-explorer-search` |
| `farming_name` | `InputState` | `sections/farming/presets.rs`: `Input` in `farming-preset-name` |
| `import.input` | `TextareaState` | `ui/import.rs`: `Textarea` in `import-input` |

`ui::input_state(placeholder, window, cx)` builds a one-line field, and `state::redraw_on_edit`
redraws the window on each of its edits.
