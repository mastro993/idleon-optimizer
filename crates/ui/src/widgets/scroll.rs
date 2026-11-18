//! Scrolling a painted element into its scroll container's view.

use gpui::{Axis, Bounds, Pixels, ScrollHandle, point};

/// Scrolls `scroll` the least that shows `item` along `axis`; true when it moved.
///
/// `item` is the element's painted bounds, as a `canvas` inside it reports
/// them. An item longer than the view shows its start.
pub fn bring_into_view(scroll: &ScrollHandle, item: Bounds<Pixels>, axis: Axis) -> bool {
    let (frame, offset) = (scroll.bounds(), scroll.offset());
    let shift = match axis {
        Axis::Horizontal => shift((item.left(), item.right()), (frame.left(), frame.right())),
        Axis::Vertical => shift((item.top(), item.bottom()), (frame.top(), frame.bottom())),
    };
    if shift != Pixels::ZERO {
        scroll.set_offset(match axis {
            Axis::Horizontal => point(offset.x + shift, offset.y),
            Axis::Vertical => point(offset.x, offset.y + shift),
        });
    }
    shift != Pixels::ZERO
}

/// How far to move the content so `(start, end)` lands inside `(first, last)`.
fn shift((start, end): (Pixels, Pixels), (first, last): (Pixels, Pixels)) -> Pixels {
    if start < first {
        first - start
    } else if end > last {
        (last - end).max(first - start)
    } else {
        Pixels::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::shift;
    use gpui::{Pixels, px};

    #[test]
    fn an_item_moves_the_least_that_shows_it() {
        let view = (px(100.), px(200.));
        assert_eq!(shift((px(120.), px(150.)), view), Pixels::ZERO);
        assert_eq!(shift((px(80.), px(110.)), view), px(20.));
        assert_eq!(shift((px(190.), px(230.)), view), px(-30.));
        // Taller than the view: its start wins.
        assert_eq!(shift((px(150.), px(400.)), view), px(-50.));
    }
}
