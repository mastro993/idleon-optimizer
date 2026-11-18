//! The app's dark palette and the world badges.
//!
//! Ely owns the component tokens; this layer only pins the app's own colors, so
//! every section reads the same values. While Ely and GPUI Kit coexist (#335),
//! the kit's theme in `data/theme.json` carries the same colors.

use crate::sections::Group;
use ely_gpui_component::theme::{
    ActiveTheme as _, ControlSize, Mode, Palette, Radius, TextSize, Theme,
};
use gpui::{
    App, FontWeight, Hsla, IntoElement, ParentElement, RenderOnce, Styled, Window, div, rgb, rgba,
};
use gpui_kit::component::{Theme as KitTheme, ThemeRegistry};

/// The kit's theme set: [`palette`]'s colors under the kit's keys.
const KIT_THEMES: &str = include_str!("../../data/theme.json");
/// The theme in [`KIT_THEMES`] the window draws with.
const KIT_THEME: &str = "Idleon Dark";

/// Draw both component libraries in the app's dark palette.
///
/// Call it after `ely_gpui_component::init` and `gpui_kit::init`.
pub(crate) fn install(cx: &mut App) {
    Theme::set_palette(Mode::Dark, Some(palette()), cx);
    Theme::set_mode_now(Mode::Dark, cx);
    let registry = ThemeRegistry::global_mut(cx);
    registry
        .load_themes_from_str(KIT_THEMES)
        .expect("the bundled kit theme parses");
    let config = registry
        .themes()
        .get(KIT_THEME)
        .cloned()
        .expect("the bundled kit theme set names its theme");
    KitTheme::update(cx, |theme| theme.apply_config(&config));
}

/// Ely's dark palette with the app's colors on top.
///
/// The ground, surfaces, borders and accent are pinned (`#1a1a2e`, `#16213e`,
/// `#0f3460`, `#00d4ff`); the remaining semantic
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

/// The fill and text of one world badge.
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

/// A world badge in its world's color, sized like Ely's `Badge`.
///
/// Ely's badge tones are semantic, while the world badges each keep a
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
    use gpui::{Context, TestAppContext};
    use gpui_kit::component::{
        ThemeSet,
        button::{Button, ButtonVariants as _},
        label::Label,
    };
    use std::rc::Rc;

    /// The kit theme as `install` applies it, outside any app.
    fn kit_theme() -> KitTheme {
        let set: ThemeSet = serde_json::from_str(KIT_THEMES).expect("theme set JSON");
        let config = set
            .themes
            .into_iter()
            .find(|theme| theme.name == KIT_THEME)
            .expect("the set names the applied theme");
        let mut theme = KitTheme::default();
        theme.apply_config(&Rc::new(config));
        theme
    }

    #[test]
    fn the_kit_theme_draws_ely_s_palette() {
        // List and table selection and text selection are left out: the kit
        // caps their alpha, so they are tints of the accent instead.
        let (theme, ely) = (kit_theme(), palette());
        assert!(theme.mode.is_dark());
        let kit = theme.colors;
        let pairs = [
            ("background", kit.background, ely.bg),
            ("foreground", kit.foreground, ely.fg),
            ("border", kit.border, ely.border),
            ("input.border", kit.input, ely.border_strong),
            ("ring", kit.ring, ely.focus),
            ("overlay", kit.overlay, ely.backdrop),
            ("accent.background", kit.accent, ely.hover),
            ("muted.background", kit.muted, ely.sunken),
            ("muted.foreground", kit.muted_foreground, ely.fg_muted),
            ("popover.background", kit.popover, ely.overlay),
            ("secondary.background", kit.secondary, ely.surface),
            ("secondary.hover.background", kit.secondary_hover, ely.hover),
            (
                "secondary.active.background",
                kit.secondary_active,
                ely.active,
            ),
            ("primary.background", kit.primary, ely.accent),
            (
                "primary.hover.background",
                kit.primary_hover,
                ely.accent_hover,
            ),
            ("primary.foreground", kit.primary_foreground, ely.on_accent),
            ("success.background", kit.success, ely.success),
            ("warning.background", kit.warning, ely.warning),
            ("danger.background", kit.danger, ely.danger),
            ("info.background", kit.info, ely.info),
            ("link", kit.link, ely.link),
            ("list.background", kit.list, ely.surface),
            ("list.hover.background", kit.list_hover, ely.hover),
            (
                "table.head.foreground",
                kit.table_head_foreground,
                ely.fg_subtle,
            ),
            ("sidebar.background", kit.sidebar, ely.sunken),
            ("sidebar.foreground", kit.sidebar_foreground, ely.fg_muted),
            ("chart.1", kit.chart_1, ely.chart[0]),
            ("chart.5", kit.chart_5, ely.chart[4]),
        ];
        for (key, kit, ely) in pairs {
            assert_eq!(kit.to_rgb(), ely.to_rgb(), "{key}");
        }
    }

    /// A kit primary `Button` and an underlined `Label`: the underline takes
    /// the label's text color, so the painted frame shows both colors. The
    /// padding keeps the test pointer, at the window origin, off the button.
    struct KitSample;

    impl gpui::Render for KitSample {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .p_8()
                .child(Button::new("kit-button").primary().label("Run"))
                .child(Label::new("Kit label").underline())
        }
    }

    #[gpui::test]
    fn kit_components_paint_the_app_colors(cx: &mut TestAppContext) {
        cx.update(|cx| {
            ely_gpui_component::init_for_tests(cx);
            gpui_kit::init(cx);
            install(cx);
        });
        let (_, cx) = cx.add_window_view(|_, _| KitSample);
        cx.run_until_parked();
        let ely = palette();
        let (quads, underlines) =
            cx.update(|window, _| (window.painted_quads(), window.painted_underlines()));
        assert!(
            quads.iter().any(
                |quad| quad.background.as_solid().map(|color| color.to_rgb())
                    == Some(ely.accent.to_rgb())
            ),
            "no quad in the accent {:?}",
            ely.accent
        );
        assert!(
            underlines
                .iter()
                .any(|underline| underline.color.to_rgb() == ely.fg.to_rgb()),
            "no label text in the foreground {:?}",
            ely.fg
        );
    }

    #[test]
    fn palette_pins_the_ground_and_accent() {
        let palette = palette();
        assert_eq!(palette.bg, rgb(0x1a1a2e).into());
        assert_eq!(palette.surface, rgb(0x16213e).into());
        assert_eq!(palette.border, rgb(0x0f3460).into());
        assert_eq!(palette.accent, rgb(0x00d4ff).into());
        assert_eq!(palette.fg, rgb(0xe0e0e0).into());
    }

    #[test]
    fn every_world_badge_keeps_its_hue() {
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
