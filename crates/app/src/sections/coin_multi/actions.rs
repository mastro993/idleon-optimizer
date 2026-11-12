//! What the window does when the coin multi section is edited or run.

use crate::state::AppState;
use gpui::Context;

impl AppState {
    /// Select the coin multiplier's character.
    pub(crate) fn set_coin_character(&mut self, character: usize, cx: &mut Context<Self>) {
        self.coin_multi.character = character;
        cx.notify();
    }
}
