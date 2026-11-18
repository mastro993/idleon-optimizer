//! The display composites GPUI Kit lacks, styled only from the kit theme.
//!
//! Everything else a screen draws comes from `gpui_kit::component`;
//! `docs/ui-kit/display.md` maps each Ely item to its kit or `idleon_ui`
//! replacement. Sizes follow the Console design (`docs/ui-kit/design.md`) in
//! pixels, so they hold when the kit theme changes the window's rem.

mod callout;
mod caption;
mod card;
mod icons;
mod scroll;
mod stat;
mod timeline;
mod tokens;

#[cfg(test)]
mod tests;

pub use callout::Callout;
pub use caption::Caption;
pub use card::{Card, CardHeader};
pub use icons::Icons;
pub use scroll::bring_into_view;
pub use stat::{KpiStrip, Statistic};
pub use timeline::{Timeline, TimelineItem};
pub use tokens::{
    PANEL_GAP, PANEL_PADDING, TEXT_BODY, TEXT_CAPTION, TEXT_KPI, TEXT_MONO, TEXT_TITLE, Tone,
};
