//! The Console type scale and spacing, and the status tones every composite reads.
//!
//! Values come from `docs/ui-kit/design.md` (§ Type, § Density and shape).
//! Colors are never here: a tone resolves to a kit theme color at render.

use gpui::{App, Hsla, Pixels, px};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::alert::AlertVariant;
use gpui_kit::component::tag::TagVariant;

/// Page title.
pub const TEXT_TITLE: Pixels = px(18.);
/// Body text, table cells and panel titles.
pub const TEXT_BODY: Pixels = px(13.);
/// A number in a cell or stepper, in the mono face.
pub const TEXT_MONO: Pixels = px(12.);
/// A KPI value, in the mono face.
pub const TEXT_KPI: Pixels = px(18.);
/// Labels, captions, units and footnotes.
pub const TEXT_CAPTION: Pixels = px(11.);
/// Padding inside a panel.
pub const PANEL_PADDING: Pixels = px(12.);
/// Gap between panels, and between the blocks inside one.
pub const PANEL_GAP: Pixels = px(8.);

/// What a mark means: the status colors plus the neutral and accent ones.
///
/// It replaces Ely's `Tone` and `Severity`; kit components take it through
/// `TagVariant::from` and `AlertVariant::from`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    #[default]
    Neutral,
    Accent,
    Info,
    Success,
    Warning,
    Danger,
}

impl Tone {
    /// The tone as text, dot or bar color on any surface.
    pub fn color(self, cx: &App) -> Hsla {
        let theme = cx.theme();
        match self {
            Self::Neutral => theme.muted_foreground,
            Self::Accent => theme.primary,
            Self::Info => theme.info,
            Self::Success => theme.success,
            Self::Warning => theme.warning,
            Self::Danger => theme.danger,
        }
    }
}

impl From<Tone> for TagVariant {
    fn from(tone: Tone) -> Self {
        match tone {
            Tone::Neutral => Self::Secondary,
            Tone::Accent => Self::Primary,
            Tone::Info => Self::Info,
            Tone::Success => Self::Success,
            Tone::Warning => Self::Warning,
            Tone::Danger => Self::Danger,
        }
    }
}

impl From<Tone> for AlertVariant {
    fn from(tone: Tone) -> Self {
        match tone {
            Tone::Neutral | Tone::Accent => Self::Default,
            Tone::Info => Self::Info,
            Tone::Success => Self::Success,
            Tone::Warning => Self::Warning,
            Tone::Danger => Self::Error,
        }
    }
}
