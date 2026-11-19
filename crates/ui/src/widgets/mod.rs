//! The display composites and form controls GPUI Kit lacks, styled only from
//! the kit theme, and the rows delegate for its virtualized `DataTable`.
//!
//! Everything else a screen draws comes from `gpui_kit::component`;
//! `docs/ui-kit/display.md` and `docs/ui-kit/forms.md` say which kit or
//! `idleon_ui` component each need takes. Sizes follow the Console design
//! (`docs/ui-kit/design.md`) in pixels, so they hold when the kit theme
//! changes the window's rem.

mod callout;
mod caption;
mod card;
mod choice;
mod drop_zone;
mod icons;
mod number;
mod scroll;
mod search;
mod stat;
mod table;
mod timeline;
mod tokens;

#[cfg(test)]
mod tests;

pub use callout::Callout;
pub use caption::Caption;
pub use card::{Card, CardHeader};
pub use choice::{Choice, ChoiceSelect};
pub use drop_zone::DropZone;
pub use icons::Icons;
pub use number::{number_state, number_value, show_number};
pub use scroll::bring_into_view;
pub use search::search_input;
pub use stat::{KpiStrip, Statistic};
pub use table::{RowColumn, RowsDelegate};
pub use timeline::{Timeline, TimelineItem};
pub use tokens::{
    PANEL_GAP, PANEL_PADDING, TEXT_BODY, TEXT_CAPTION, TEXT_KPI, TEXT_MONO, TEXT_TITLE, Tone,
};
