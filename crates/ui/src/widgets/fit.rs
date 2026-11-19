//! Widths that fit a small kit control to its content, and the label every
//! field carries.
//!
//! The kit `Select` and `NumberInput` fill their parent, so a row of them gives
//! each one a box (`docs/ui-kit/forms.md`). These widths size that box to what
//! the control shows instead of a guess, so a long option or a wide number
//! never truncates.

use gpui::{
    AnyElement, App, Entity, Hsla, IntoElement, ParentElement, Pixels, RenderOnce, SharedString,
    Styled, TextRun, Window, div, font, px, rems,
};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::input::InputState;

use super::Caption;

/// The chrome of a small kit `Select` around its label: the 8 px padding on
/// each side, the 1 px border, the 4 px gap and the 14 px caret.
const SELECT_CHROME: Pixels = px(2. * 8. + 2. + 4. + 14.);
/// The chrome of a small kit `NumberInput` around its text: two 24 px step
/// buttons, the field's 8 px padding on each side and the 1 px border.
const NUMBER_CHROME: Pixels = px(2. * 24. + 2. * 8. + 2.);
/// Room for the caret and subpixel rounding, so a fitted label never clips.
const SLACK: Pixels = px(4.);
/// The narrowest number field holds this, so a one-digit value is still easy
/// to click and does not jump as it grows to two or three digits.
const NUMBER_FLOOR: &str = "0000";

/// The width `text` takes in a small kit control: the theme's UI face at the
/// kit's small text size (0.875 rem).
fn control_text_width(text: &str, window: &mut Window, cx: &App) -> Pixels {
    let size = rems(0.875).to_pixels(window.rem_size());
    let run = TextRun {
        len: text.len(),
        font: font(cx.theme().font_family.clone()),
        color: Hsla::default(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    window
        .text_system()
        .shape_line(SharedString::from(text.to_owned()), size, &[run], None)
        .width
}

/// The width of a small kit `Select` whose widest option reads `label`.
pub(super) fn select_width(label: &str, window: &mut Window, cx: &App) -> Pixels {
    control_text_width(label, window, cx) + SELECT_CHROME + SLACK
}

/// The width of a small kit `NumberInput` showing what `field` holds now,
/// never narrower than four digits. Call it from render, next to
/// `show_number`, so the field grows with its value.
pub fn number_width(field: &Entity<InputState>, window: &mut Window, cx: &App) -> Pixels {
    let value = field.read(cx).value();
    control_text_width(&value, window, cx).max(control_text_width(NUMBER_FLOOR, window, cx))
        + NUMBER_CHROME
        + SLACK
}

/// A form field: its label over the control (`docs/ui-kit/design.md`). It
/// never grows past its row, so a control boxed to its content is capped by
/// the row instead of overflowing it.
pub fn labeled(label: impl Into<SharedString>, control: impl IntoElement) -> Labeled {
    Labeled {
        label: label.into(),
        control: control.into_any_element(),
    }
}

/// A control under its label; built by [`labeled`].
#[derive(IntoElement)]
pub struct Labeled {
    label: SharedString,
    control: AnyElement,
}

impl RenderOnce for Labeled {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .flex_none()
            .max_w_full()
            .gap_1()
            .child(Caption::new(self.label).text_color(cx.theme().muted_foreground))
            .child(self.control)
    }
}
