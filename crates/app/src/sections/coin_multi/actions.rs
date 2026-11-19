//! What the window does when the coin multi section is edited or run.

use crate::state::AppState;
use gpui::Context;

impl AppState {
    /// Select the coin multiplier's character.
    pub(crate) fn set_coin_character(&mut self, character: usize, cx: &mut Context<Self>) {
        self.coin_multi.character = character;
        self.coin_multi.refresh(cx);
        cx.notify();
    }

    /// Take the damage field's text; the overkill tier and the total follow it.
    pub(crate) fn set_coin_damage(&mut self, damage: String, cx: &mut Context<Self>) {
        if self.coin_multi.damage == damage {
            return;
        }
        self.coin_multi.damage = damage;
        self.coin_multi.refresh(cx);
        cx.notify();
    }
}
