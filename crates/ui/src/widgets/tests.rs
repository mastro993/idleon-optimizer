//! Each composite rendered through the kit's `Root`, as the app's window draws it.

use gpui::{
    AnyElement, App, AssetSource as _, Hsla, IntoElement, ParentElement as _, Render,
    TestAppContext, VisualTestContext, Window, div, prelude::*,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::alert::AlertVariant;
use gpui_kit::component::tag::TagVariant;
use gpui_kit::component::{ActiveTheme as _, Sizable as _};

use super::*;

struct Sample(fn() -> AnyElement);

impl Render for Sample {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        div().size_full().child((self.0)())
    }
}

fn open(cx: &mut TestAppContext, sample: fn() -> AnyElement) -> &mut VisualTestContext {
    cx.update(gpui_kit::init);
    let (window, _) = cx
        .update(|cx| {
            gpui_kit::open_window(gpui::WindowOptions::default(), cx, |_, cx| {
                cx.new(|_| Sample(sample))
            })
        })
        .expect("test window");
    let cx = VisualTestContext::from_window(window, cx).into_mut();
    cx.run_until_parked();
    cx
}

fn color(cx: &mut VisualTestContext, pick: fn(&App) -> Hsla) -> Hsla {
    cx.update(|_, cx| pick(cx))
}

fn painted_fill(cx: &mut VisualTestContext, fill: Hsla) -> bool {
    cx.update(|window, _| {
        window
            .painted_quads()
            .iter()
            .any(|quad| quad.background.as_solid() == Some(fill))
    })
}

fn painted_border(cx: &mut VisualTestContext, border: Hsla) -> bool {
    cx.update(|window, _| {
        window
            .painted_quads()
            .iter()
            .any(|quad| quad.border_color == border && quad.border_widths.left.0 > 0.)
    })
}

#[gpui::test]
fn a_caption_lays_out_its_text(cx: &mut TestAppContext) {
    let cx = open(cx, || {
        div()
            .debug_selector(|| "caption".into())
            .child(Caption::new("Per hour, at the current rates"))
            .into_any_element()
    });
    let bounds = cx.debug_bounds("caption").expect("caption drawn");
    assert!(bounds.size.height > gpui::px(0.));
}

#[gpui::test]
fn a_card_draws_its_panel_header_body_and_footer(cx: &mut TestAppContext) {
    let cx = open(cx, || {
        Card::new()
            .header(
                CardHeader::new("Progress")
                    .description("The run in flight")
                    .action(
                        div()
                            .debug_selector(|| "card-action".into())
                            .child("Cancel"),
                    ),
            )
            .child(div().debug_selector(|| "card-body".into()).child("Body"))
            .footer(
                div()
                    .debug_selector(|| "card-footer".into())
                    .child("Footer"),
            )
            .into_any_element()
    });
    let border = color(cx, |cx| cx.theme().border);
    let surface = color(cx, |cx| cx.theme().group_box);
    assert!(painted_border(cx, border), "no panel border");
    assert!(painted_fill(cx, surface), "no panel surface");
    let action = cx.debug_bounds("card-action").expect("header action");
    let body = cx.debug_bounds("card-body").expect("body");
    let footer = cx.debug_bounds("card-footer").expect("footer");
    assert!(
        action.bottom() <= body.top(),
        "the header sits above the body"
    );
    assert!(
        body.bottom() <= footer.top(),
        "the footer sits under the body"
    );
    assert!(action.left() > body.left(), "the action trails the title");
}

#[gpui::test]
fn a_statistic_sizes_its_value(cx: &mut TestAppContext) {
    fn figure() -> Statistic {
        Statistic::new("exp-rate", "EXP / hr", "14.32B")
            .suffix("/hr")
            .delta("+38.6%", Tone::Success)
            .caption("Best placement")
    }
    let cx = open(cx, || {
        div()
            .flex()
            .items_start()
            .child(
                div()
                    .debug_selector(|| "statistic-medium".into())
                    .child(figure().tone(Tone::Accent)),
            )
            .child(
                div()
                    .debug_selector(|| "statistic-large".into())
                    .child(figure().large()),
            )
            .into_any_element()
    });
    let medium = cx
        .debug_bounds("statistic-medium")
        .expect("statistic drawn");
    let large = cx.debug_bounds("statistic-large").expect("statistic drawn");
    assert!(
        large.size.height > medium.size.height,
        "a large value is taller"
    );
}

#[gpui::test]
fn a_kpi_strip_splits_its_figures_evenly(cx: &mut TestAppContext) {
    let cx = open(cx, || {
        KpiStrip::new()
            .w(gpui::px(600.))
            .child(
                div()
                    .debug_selector(|| "kpi-high".into())
                    .child(Statistic::new("high", "High value", "12")),
            )
            .child(
                div()
                    .debug_selector(|| "kpi-moderate".into())
                    .child(Statistic::new("moderate", "Moderate", "1,204")),
            )
            .child(
                div()
                    .debug_selector(|| "kpi-low".into())
                    .child(Statistic::new("low", "Low or none", "3")),
            )
            .into_any_element()
    });
    let cells = ["kpi-high", "kpi-moderate", "kpi-low"]
        .map(|cell| cx.debug_bounds(cell).expect("cell drawn"));
    assert!(cells[0].right() <= cells[1].left() && cells[1].right() <= cells[2].left());
    assert_eq!(cells[0].size.width, cells[2].size.width);
    let border = color(cx, |cx| cx.theme().border);
    assert!(painted_border(cx, border), "no hairline");
}

#[gpui::test]
fn a_callout_marks_its_tone_on_the_sunken_surface(cx: &mut TestAppContext) {
    let cx = open(cx, || {
        Callout::new(Tone::Warning, "Jar Production Line is not owned")
            .child(Caption::new("Its jars are left out of the plan."))
            .into_any_element()
    });
    let warning = color(cx, |cx| cx.theme().warning);
    let sunken = color(cx, |cx| cx.theme().sidebar);
    assert!(painted_fill(cx, warning), "no status dot");
    assert!(painted_fill(cx, sunken), "no sunken row");
}

#[gpui::test]
fn a_timeline_stacks_its_events_with_toned_dots(cx: &mut TestAppContext) {
    let cx = open(cx, || {
        Timeline::new()
            .item(
                TimelineItem::new("Focused anchor slot 3")
                    .time("1.25s")
                    .tone(Tone::Success)
                    .child(div().debug_selector(|| "first".into()).child("Revived")),
            )
            .item(
                TimelineItem::new("Footprint slot 7")
                    .time("4.50s")
                    .child(div().debug_selector(|| "second".into()).child("Lost")),
            )
            .into_any_element()
    });
    let success = color(cx, |cx| cx.theme().success);
    assert!(painted_fill(cx, success), "no success dot");
    let first = cx.debug_bounds("first").expect("first event");
    let second = cx.debug_bounds("second").expect("second event");
    assert!(first.bottom() <= second.top());
}

#[test]
fn the_icon_source_serves_the_kit_and_the_app_icons() {
    for icon in [
        IconName::Check,
        IconName::ChevronRight,
        IconName::SearchX,
        IconName::Upload,
        IconName::X,
        IconName::Trash,
    ] {
        let path = icon.path();
        let data = Icons.load(&path).expect("the source answers");
        assert!(data.is_some_and(|data| !data.is_empty()), "{path}");
    }
    let listed = Icons.list("icons/").expect("the source lists");
    assert!(
        listed
            .iter()
            .any(|path| path.as_ref() == IconName::SearchX.path().as_ref())
    );
    assert!(
        listed
            .iter()
            .any(|path| path.as_ref() == IconName::Check.path().as_ref())
    );
}

#[test]
fn a_tone_picks_the_matching_kit_variant() {
    assert!(matches!(
        TagVariant::from(Tone::Neutral),
        TagVariant::Secondary
    ));
    assert!(matches!(
        TagVariant::from(Tone::Accent),
        TagVariant::Primary
    ));
    assert!(matches!(TagVariant::from(Tone::Danger), TagVariant::Danger));
    assert_eq!(AlertVariant::from(Tone::Danger), AlertVariant::Error);
    assert_eq!(AlertVariant::from(Tone::Accent), AlertVariant::Default);
    assert_eq!(AlertVariant::from(Tone::Warning), AlertVariant::Warning);
}
