//! Page-level history: the places opened in this window, oldest first, and
//! where the window stands among them. Tabs inside a section are not places,
//! and the history is never saved.

use super::Route;

/// The most places kept; opening one more forgets the oldest.
const CAP: usize = 50;

/// The places opened in this window, oldest first, with a current position.
#[derive(Debug)]
pub(crate) struct History {
    places: Vec<Route>,
    current: usize,
}

impl History {
    /// A history holding only the place the window opens on.
    pub(crate) fn new(place: Route) -> Self {
        Self {
            places: vec![place],
            current: 0,
        }
    }

    /// The place showing.
    pub(crate) fn current(&self) -> &Route {
        &self.places[self.current]
    }

    /// Every place, oldest first.
    pub(crate) fn places(&self) -> &[Route] {
        &self.places
    }

    /// Where the window stands in [`Self::places`].
    pub(crate) fn position(&self) -> usize {
        self.current
    }

    /// Open a place as a new step, dropping any forward steps. Opening the
    /// place already showing adds nothing. Returns whether the place changed.
    pub(crate) fn open(&mut self, place: Route) -> bool {
        if *self.current() == place {
            return false;
        }
        self.places.truncate(self.current + 1);
        self.places.push(place);
        if self.places.len() > CAP {
            self.places.remove(0);
        }
        self.current = self.places.len() - 1;
        true
    }

    /// Step back one place. Returns whether it moved.
    pub(crate) fn go_back(&mut self) -> bool {
        self.current > 0 && self.go_to(self.current - 1)
    }

    /// Step forward one place. Returns whether it moved.
    pub(crate) fn go_forward(&mut self) -> bool {
        self.go_to(self.current + 1)
    }

    /// Jump to a place by its index, as the history list does, without adding
    /// a step. Returns whether it moved.
    pub(crate) fn go_to(&mut self, index: usize) -> bool {
        if index == self.current || index >= self.places.len() {
            return false;
        }
        self.current = index;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(id: &str) -> Route {
        Route::open(id)
    }

    fn ids(history: &History) -> Vec<String> {
        history
            .places()
            .iter()
            .map(|place| match place {
                Route::Dashboard => "home".to_string(),
                Route::Section(section) => section.id.to_string(),
                Route::Missing(id) => format!("missing:{id}"),
            })
            .collect()
    }

    #[test]
    fn a_new_history_shows_the_launch_place_with_nowhere_to_go() {
        let mut history = History::new(Route::Dashboard);
        assert_eq!(history.current(), &Route::Dashboard);
        assert!(!history.go_back());
        assert!(!history.go_forward());
        assert_eq!(history.current(), &Route::Dashboard);
    }

    #[test]
    fn back_and_forward_step_through_the_opened_places() {
        let mut history = History::new(Route::Dashboard);
        assert!(history.open(section("farming")));
        assert!(history.open(section("no-such-section")));
        assert_eq!(
            ids(&history),
            ["home", "farming", "missing:no-such-section"]
        );

        assert!(history.go_back());
        assert_eq!(history.current(), &section("farming"));
        assert!(history.go_back());
        assert_eq!(history.current(), &Route::Dashboard);
        assert!(!history.go_back(), "back stops at the oldest place");

        assert!(history.go_forward());
        assert!(history.go_forward());
        assert_eq!(history.current(), &section("no-such-section"));
        assert!(!history.go_forward(), "forward stops at the newest place");
        assert_eq!(history.places().len(), 3, "moving adds no steps");
    }

    #[test]
    fn opening_the_place_already_showing_adds_nothing() {
        let mut history = History::new(Route::Dashboard);
        history.open(section("farming"));
        assert!(!history.open(section("farming")));
        assert_eq!(ids(&history), ["home", "farming"]);
    }

    #[test]
    fn opening_a_place_after_going_back_drops_the_forward_steps() {
        let mut history = History::new(Route::Dashboard);
        history.open(section("farming"));
        history.open(section("sailing"));
        history.go_back();
        history.go_back();

        assert!(history.open(section("damage")));
        assert_eq!(ids(&history), ["home", "damage"]);
        assert_eq!(history.position(), 1);
        assert!(!history.go_forward());
    }

    #[test]
    fn the_history_keeps_the_newest_fifty_places() {
        let mut history = History::new(Route::Dashboard);
        for step in 1..=60 {
            let place = if step % 2 == 0 {
                Route::Dashboard
            } else {
                section("farming")
            };
            history.open(place);
        }
        assert_eq!(history.places().len(), 50);
        assert_eq!(history.position(), 49);
        assert_eq!(history.current(), &Route::Dashboard);
        assert_eq!(
            history.places()[0],
            section("farming"),
            "the oldest ten went"
        );
    }

    #[test]
    fn the_history_list_jumps_to_a_place_without_adding_a_step() {
        let mut history = History::new(Route::Dashboard);
        history.open(section("farming"));
        history.open(section("sailing"));

        assert!(history.go_to(0));
        assert_eq!(history.current(), &Route::Dashboard);
        assert!(!history.go_to(0), "the place already showing");
        assert!(!history.go_to(3), "past the newest place");
        assert_eq!(history.position(), 0);
        assert!(history.go_to(2));
        assert_eq!(ids(&history), ["home", "farming", "sailing"]);
    }
}
