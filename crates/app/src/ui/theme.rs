//! The Console theme (`docs/ui-kit/design.md`), its mono face and the world badges.
//!
//! `data/theme.json` is the one source of the app's colors: the kit draws it
//! directly, and while Ely and GPUI Kit coexist (#335) Ely's `Palette` is
//! derived from the same tokens. Both follow the system's light or dark
//! appearance.

use crate::sections::Group;
use ely_gpui_component::theme::{Mode, Palette, Theme};
use gpui::{
    App, FontWeight, Hsla, IntoElement, ParentElement, RenderOnce, Styled, Window,
    WindowAppearance, rgb,
};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{
    ActiveTheme as _, Sizable as _, Theme as KitTheme, ThemeConfig, ThemeRegistry,
};
use idleon_ui::widgets::TEXT_CAPTION;
use std::borrow::Cow;
use std::rc::Rc;

/// The kit's theme set: the Console theme in its dark and light modes.
const KIT_THEMES: &str = include_str!("../../data/theme.json");
/// The theme in [`KIT_THEMES`] a dark window draws with.
const DARK_THEME: &str = "Idleon Console Dark";
/// The theme in [`KIT_THEMES`] a light window draws with.
const LIGHT_THEME: &str = "Idleon Console Light";

/// JetBrains Mono, the theme's `mono_font.family`, in the weights the design
/// sets numbers in (`fonts/JetBrainsMono-OFL.txt` is its license). The kit
/// uses a named family as-is, so it must be registered before text is laid out.
const MONO_FONTS: [&[u8]; 3] = [
    include_bytes!("../../fonts/JetBrainsMono-Regular.ttf"),
    include_bytes!("../../fonts/JetBrainsMono-Medium.ttf"),
    include_bytes!("../../fonts/JetBrainsMono-SemiBold.ttf"),
];

/// Register the mono face and draw both component libraries in the Console
/// theme, in the system's current mode.
///
/// Call it after `ely_gpui_component::init` and `gpui_kit::init`.
pub(crate) fn install(cx: &mut App) {
    cx.text_system()
        .add_fonts(MONO_FONTS.iter().map(|font| Cow::Borrowed(*font)).collect())
        .expect("the bundled mono font loads");
    let registry = ThemeRegistry::global_mut(cx);
    registry
        .load_themes_from_str(KIT_THEMES)
        .expect("the bundled kit theme parses");
    let config = |name| {
        registry
            .themes()
            .get(name)
            .cloned()
            .expect("the bundled kit theme set names its themes")
    };
    let (dark, light) = (config(DARK_THEME), config(LIGHT_THEME));
    Theme::set_palette(Mode::Dark, Some(palette(&dark)), cx);
    Theme::set_palette(Mode::Light, Some(palette(&light)), cx);
    KitTheme::global_mut(cx).dark_theme = dark;
    KitTheme::global_mut(cx).light_theme = light;
    show(cx.window_appearance(), cx);
}

/// Follow the system's appearance in `window` from now on.
pub(crate) fn follow_system(window: &Window) {
    window
        .observe_window_appearance(|window, cx| show(window.appearance(), cx))
        .detach();
}

/// Draw both libraries in `appearance`'s mode.
fn show(appearance: WindowAppearance, cx: &mut App) {
    Theme::set_mode_now(appearance.into(), cx);
    KitTheme::change(appearance, None, cx);
}

/// Ely's palette for `config`'s mode, from the kit theme's tokens
/// (`docs/ui-kit/design.md`, § Color tokens). Colors the Console theme has no
/// token for (disabled text, shadows, code and terminal colors) stay Ely's.
fn palette(config: &Rc<ThemeConfig>) -> Palette {
    let mut theme = KitTheme::default();
    theme.apply_config(config);
    let kit = theme.colors;
    let mut palette = if theme.mode.is_dark() {
        Palette::dark(false)
    } else {
        Palette::light(false)
    };
    palette.bg = kit.background;
    palette.surface = kit.table;
    palette.sunken = kit.sidebar;
    palette.overlay = kit.popover;
    palette.hover = kit.accent;
    palette.active = kit.secondary_active;
    palette.border = kit.border;
    palette.border_strong = kit.input;
    palette.fg = kit.foreground;
    palette.fg_muted = kit.muted_foreground;
    palette.fg_subtle = kit.table_head_foreground;
    palette.accent = kit.primary;
    palette.accent_hover = kit.primary_hover;
    palette.on_accent = kit.primary_foreground;
    palette.focus = kit.ring;
    palette.link = kit.link;
    palette.selection = kit.selection;
    palette.success = kit.success;
    palette.warning = kit.warning;
    palette.danger = kit.danger;
    palette.info = kit.info;
    palette.success_subtle = kit.success.opacity(WORLD_FILL);
    palette.warning_subtle = kit.warning.opacity(WORLD_FILL);
    palette.danger_subtle = kit.danger.opacity(WORLD_FILL);
    palette.info_subtle = kit.info.opacity(WORLD_FILL);
    palette.backdrop = kit.overlay;
    palette.tooltip_bg = kit.popover;
    palette.tooltip_fg = kit.popover_foreground;
    palette.chart[..5].copy_from_slice(&[
        kit.chart_1,
        kit.chart_2,
        kit.chart_3,
        kit.chart_4,
        kit.chart_5,
    ]);
    palette
}

/// One world's color in the active mode: a badge's text, and at
/// [`WORLD_FILL`] and [`WORLD_OUTLINE`] alpha its fill and outline.
///
/// Not kit theme keys: the app keeps one hue per world, darker in light mode
/// so it reads on white (`docs/ui-kit/design.md`, § World badges).
pub(crate) fn world_color(group: Group, cx: &App) -> Hsla {
    world_hue(group, cx.theme().is_dark())
}

fn world_hue(group: Group, dark: bool) -> Hsla {
    let (dark_hue, light_hue) = match group {
        Group::General => (0xa1a1aa, 0x52525b),
        Group::W2 => (0x60a5fa, 0x1d4ed8),
        Group::W3 => (0x4ade80, 0x166534),
        Group::W4 => (0xc084fc, 0x7e22ce),
        Group::W5 => (0x818cf8, 0x4f46e5),
        Group::W6 => (0xfb7185, 0xbe123c),
        Group::W7 => (0xfbbf24, 0x92400e),
        Group::Tools => (0x5eead4, 0x115e59),
    };
    rgb(if dark { dark_hue } else { light_hue }).into()
}

/// A world badge's fill alpha, and the soft fill behind a status in Ely.
const WORLD_FILL: f32 = 0.14;
/// A world badge's outline alpha.
const WORLD_OUTLINE: f32 = 0.3;

/// A world badge: a small kit `Tag` in its world's color.
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
        let color = world_color(self.group, cx);
        Tag::custom(
            color.opacity(WORLD_FILL),
            color,
            color.opacity(WORLD_OUTLINE),
        )
        .small()
        .flex_none()
        .text_size(TEXT_CAPTION)
        .font_weight(FontWeight::MEDIUM)
        .child(self.group.badge())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{AppContext as _, Context, TestAppContext, div};
    use gpui_kit::component::{
        ThemeSet,
        button::{Button, ButtonVariants as _},
        label::Label,
    };

    /// The named theme of the bundled set, as `install` registers it.
    fn config(name: &str) -> Rc<ThemeConfig> {
        let set: ThemeSet = serde_json::from_str(KIT_THEMES).expect("theme set JSON");
        let config = set
            .themes
            .into_iter()
            .find(|theme| theme.name == name)
            .expect("the set names the theme");
        Rc::new(config)
    }

    /// The kit theme `config` resolves to, outside any app.
    fn kit_theme(name: &str) -> KitTheme {
        let mut theme = KitTheme::default();
        theme.apply_config(&config(name));
        theme
    }

    /// WCAG 2.x contrast ratio of two opaque colors.
    fn contrast(a: Hsla, b: Hsla) -> f32 {
        let luminance = |color: Hsla| {
            let rgba = color.to_rgb();
            let channel = |c: f32| {
                if c <= 0.039_28 {
                    c / 12.92
                } else {
                    ((c + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * channel(rgba.r) + 0.7152 * channel(rgba.g) + 0.0722 * channel(rgba.b)
        };
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn the_set_has_a_dark_and_a_light_console_theme() {
        assert!(kit_theme(DARK_THEME).mode.is_dark());
        assert!(!kit_theme(LIGHT_THEME).mode.is_dark());
    }

    #[test]
    fn ely_draws_the_kit_tokens_in_both_modes() {
        for name in [DARK_THEME, LIGHT_THEME] {
            let (kit, ely) = (kit_theme(name).colors, palette(&config(name)));
            let pairs = [
                ("background", kit.background, ely.bg),
                ("surface", kit.table, ely.surface),
                ("sunken", kit.sidebar, ely.sunken),
                ("foreground", kit.foreground, ely.fg),
                ("muted", kit.muted_foreground, ely.fg_muted),
                ("subtle", kit.table_head_foreground, ely.fg_subtle),
                ("border", kit.border, ely.border),
                ("accent", kit.primary, ely.accent),
                ("focus", kit.ring, ely.focus),
                ("danger", kit.danger, ely.danger),
                ("chart.5", kit.chart_5, ely.chart[4]),
            ];
            for (token, kit, ely) in pairs {
                assert_eq!(kit.to_rgb(), ely.to_rgb(), "{name}: {token}");
            }
        }
    }

    #[test]
    fn every_text_color_reads_on_every_surface() {
        // `docs/ui-kit/design.md`, § Contrast: AA text, 4.5:1, in both modes.
        for name in [DARK_THEME, LIGHT_THEME] {
            let theme = kit_theme(name);
            let kit = theme.colors;
            let surfaces = [
                ("background", kit.background),
                ("surface", kit.table),
                ("elevated", kit.muted),
                ("sidebar", kit.sidebar),
                ("hover", kit.accent),
            ];
            let mut texts = vec![
                ("foreground", kit.foreground),
                ("muted", kit.muted_foreground),
                ("subtle", kit.table_head_foreground),
                ("accent", kit.primary),
                ("success", kit.success),
                ("warning", kit.warning),
                ("danger", kit.danger),
                ("info", kit.info),
            ];
            texts.extend(
                Group::ALL.map(|group| (group.badge(), world_hue(group, theme.mode.is_dark()))),
            );
            for (surface, ground) in surfaces {
                for (text, color) in &texts {
                    let ratio = contrast(*color, ground);
                    assert!(ratio >= 4.5, "{name}: {text} on {surface} is {ratio:.2}:1");
                }
            }
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
    fn kit_components_paint_the_console_colors_and_both_libraries_switch_mode(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            ely_gpui_component::init_for_tests(cx);
            gpui_kit::init(cx);
            install(cx);
            show(WindowAppearance::Dark, cx);
        });
        // Through the kit's Root, as the app's window opens.
        let (window, _) = cx
            .update(|cx| {
                gpui_kit::open_window(gpui::WindowOptions::default(), cx, |_, cx| {
                    cx.new(|_| KitSample)
                })
            })
            .expect("test window");
        let cx = &mut gpui::VisualTestContext::from_window(window, cx);
        cx.run_until_parked();
        let dark = kit_theme(DARK_THEME).colors;
        let (quads, underlines, rem) = cx.update(|window, _| {
            (
                window.painted_quads(),
                window.painted_underlines(),
                window.rem_size(),
            )
        });
        // The Root sets the rem to the kit's `font.size` every frame; Ely sizes
        // everything in rems of 16 px, so any other value rescales the app.
        assert_eq!(rem, gpui::px(16.0));
        assert!(
            quads.iter().any(
                |quad| quad.background.as_solid().map(|color| color.to_rgb())
                    == Some(dark.primary.to_rgb())
            ),
            "no quad in the accent {:?}",
            dark.primary
        );
        assert!(
            underlines
                .iter()
                .any(|underline| underline.color.to_rgb() == dark.foreground.to_rgb()),
            "no label text in the foreground {:?}",
            dark.foreground
        );

        let light = kit_theme(LIGHT_THEME).colors;
        cx.update(|_, cx| show(WindowAppearance::Light, cx));
        cx.update(|_, cx| {
            assert!(!cx.theme().is_dark());
            assert_eq!(cx.theme().background.to_rgb(), light.background.to_rgb());
            let ely = ely_gpui_component::theme::ActiveTheme::theme(cx);
            assert_eq!(ely.mode(), Mode::Light);
            assert_eq!(ely.colors.bg.to_rgb(), light.background.to_rgb());
            assert_eq!(
                world_color(Group::W7, cx).to_rgb(),
                world_hue(Group::W7, false).to_rgb()
            );
        });
    }
}
