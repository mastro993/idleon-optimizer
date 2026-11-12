//! The app's dark palette and the upstream world badges.
//!
//! Ely owns the component tokens; this layer only pins the colors the upstream
//! `index.html` and `site-nav.js` use, so every section reads the same values.

use crate::sections::Group;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize, Palette, Radius, TextSize};
use gpui::{
    App, FontWeight, Hsla, IntoElement, ParentElement, RenderOnce, Styled, Window, div, rgb, rgba,
};

/// Ely's dark palette with the upstream site colors on top.
///
/// The ground, surfaces, borders and accent come from `index.html`
/// (`#1a1a2e`, `#16213e`, `#0f3460`, `#00d4ff`); the remaining semantic
/// colors stay Ely's so success, warning and danger keep their contrast.
pub(crate) fn palette() -> Palette {
    let mut palette = Palette::dark(false);
    palette.bg = rgb(0x1a1a2e).into();
    palette.surface = rgb(0x16213e).into();
    palette.sunken = rgb(0x121a30).into();
    palette.overlay = rgb(0x16213e).into();
    palette.hover = rgb(0x1a2a4e).into();
    palette.active = rgb(0x22345c).into();
    palette.border = rgb(0x0f3460).into();
    palette.border_strong = rgb(0x33415b).into();
    palette.fg = rgb(0xe0e0e0).into();
    palette.fg_muted = rgb(0xa0a0a0).into();
    palette.fg_subtle = rgb(0x949494).into();
    palette.fg_disabled = rgb(0x5c6a80).into();
    palette.accent = rgb(0x00d4ff).into();
    palette.accent_hover = rgb(0x4dd0e1).into();
    palette.on_accent = rgb(0x0d1b2a).into();
    palette.focus = rgb(0x4dd0e1).into();
    palette.link = rgb(0x4dd0e1).into();
    palette.selection = rgba(0x4dd0e188).into();
    palette.tooltip_bg = rgb(0x0f3460).into();
    palette.tooltip_fg = rgb(0xe0e0e0).into();
    palette.paper = rgb(0xe0e0e0).into();
    palette.ink = rgb(0x16213e).into();
    palette.shadow = rgb(0x000000).into();
    palette
}

/// The fill and text of one upstream world badge.
pub(crate) fn world_badge_colors(group: Group) -> (Hsla, Hsla) {
    let (fill, text) = match group {
        Group::General => (0x78909c44, 0xb0bec5),
        Group::W2 => (0x0288d144, 0x81d4fa),
        Group::W3 => (0x2e7d3244, 0x66bb6a),
        Group::W4 => (0x8e24aa44, 0xce93d8),
        Group::W5 => (0x5c6bc044, 0x9fa8da),
        Group::W6 => (0xef535044, 0xef9a9a),
        Group::W7 => (0xff980044, 0xffcc80),
        Group::Tools => (0x78909c44, 0xb0bec5),
    };
    (rgba(fill).into(), rgb(text).into())
}

/// A world badge in the upstream color, sized like Ely's `Badge`.
///
/// Ely's badge tones are semantic, while the upstream world badges each keep a
/// hue; this is the one specialization the dashboard and page header share.
#[derive(IntoElement)]
pub(crate) struct WorldBadge {
    group: Group,
}

impl WorldBadge {
    pub(crate) fn new(group: Group) -> Self {
        Self { group }
    }
}

impl RenderOnce for WorldBadge {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let (fill, text) = world_badge_colors(self.group);
        div()
            .flex()
            .flex_none()
            .items_center()
            .px_1p5()
            .h(theme.control_height(ControlSize::Sm) * 0.75)
            .rounded(theme.radius(Radius::Sm))
            .bg(fill)
            .text_size(theme.text_size(TextSize::Xs))
            .font_weight(FontWeight::MEDIUM)
            .text_color(text)
            .child(self.group.badge())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_pins_the_upstream_ground_and_accent() {
        let palette = palette();
        assert_eq!(palette.bg, rgb(0x1a1a2e).into());
        assert_eq!(palette.surface, rgb(0x16213e).into());
        assert_eq!(palette.border, rgb(0x0f3460).into());
        assert_eq!(palette.accent, rgb(0x00d4ff).into());
        assert_eq!(palette.fg, rgb(0xe0e0e0).into());
    }

    #[test]
    fn every_world_badge_keeps_its_upstream_hue() {
        let expected = [
            (Group::General, 0x78909c44, 0xb0bec5),
            (Group::W2, 0x0288d144, 0x81d4fa),
            (Group::W3, 0x2e7d3244, 0x66bb6a),
            (Group::W4, 0x8e24aa44, 0xce93d8),
            (Group::W5, 0x5c6bc044, 0x9fa8da),
            (Group::W6, 0xef535044, 0xef9a9a),
            (Group::W7, 0xff980044, 0xffcc80),
            (Group::Tools, 0x78909c44, 0xb0bec5),
        ];
        for (group, fill, text) in expected {
            assert_eq!(
                world_badge_colors(group),
                (rgba(fill).into(), rgb(text).into())
            );
        }
    }
}
