//! The Console theme (`docs/ui-kit/design.md`), its mono face and the world badges.
//!
//! `data/theme.json` is the one source of the app's colors, drawn by the kit
//! in the system's light or dark appearance.

use crate::sections::Group;
use gpui::{
    App, FontWeight, Hsla, IntoElement, ParentElement, RenderOnce, Styled, Window,
    WindowAppearance, rgb,
};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, Theme, ThemeRegistry};
use idleon_ui::widgets::TEXT_CAPTION;
use std::borrow::Cow;

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

/// Register the mono face and draw the kit in the Console theme, in the
/// system's current mode.
///
/// Call it after `gpui_kit::init`.
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
    Theme::global_mut(cx).dark_theme = dark;
    Theme::global_mut(cx).light_theme = light;
    show(cx.window_appearance(), cx);
}

/// Follow the system's appearance in `window` from now on.
pub(crate) fn follow_system(window: &Window) {
    window
        .observe_window_appearance(|window, cx| show(window.appearance(), cx))
        .detach();
}

/// Draw the kit in `appearance`'s mode.
fn show(appearance: WindowAppearance, cx: &mut App) {
    Theme::change(appearance, None, cx);
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

/// A world badge's fill alpha.
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
        ThemeConfig, ThemeSet,
        button::{Button, ButtonVariants as _},
        label::Label,
    };
    use std::rc::Rc;

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
    fn kit_theme(name: &str) -> Theme {
        let mut theme = Theme::default();
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
                ("segmented track", kit.tab_bar_segmented),
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
    fn kit_components_paint_the_console_colors_and_switch_mode(cx: &mut TestAppContext) {
        cx.update(|cx| {
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
        // The Root sets the rem to the kit's `font.size` every frame; the app's
        // spacing and the kit's component heights are rems of 16 px, so any
        // other value rescales the app.
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
            assert_eq!(
                world_color(Group::W7, cx).to_rgb(),
                world_hue(Group::W7, false).to_rgb()
            );
        });
    }
}
