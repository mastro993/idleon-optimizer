//! The result view's timeline: the projection's samples as the segments between
//! its events.
//!
//! The rate is constant between events, so a segment's EXP is the rate its
//! opening sample carries times its length. Adding back the tournament bonuses
//! gives the projection's total EXP, which is how the timeline and the EXP/hour
//! chart are checked against the samples they are drawn from.

use crate::sim::{Event, Projection};

/// One segment of the run: what it ground out, and what it ended on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Phase {
    /// The event that opened the segment.
    pub event: Event,
    /// Hours from the run's start.
    pub start: f64,
    /// How long the segment ran.
    pub hours: f64,
    /// Research level when it opened and at its end.
    pub from_level: u32,
    pub to_level: u32,
    /// Insight levels collected over it.
    pub insight: u32,
    /// The rate it held throughout.
    pub rate: f64,
    /// EXP earned over it at that rate.
    pub exp: f64,
    /// EXP a tournament injected at its opening, without elapsed time.
    pub bonus: f64,
}

impl Phase {
    /// The segment's end, in hours from the run's start.
    pub fn end(&self) -> f64 {
        self.start + self.hours
    }
}

/// The projection as the segments between its samples, oldest first.
pub fn phases(projection: &Projection) -> Vec<Phase> {
    projection
        .samples
        .windows(2)
        .map(|pair| {
            let (open, close) = (pair[0], pair[1]);
            let hours = close.hours - open.hours;
            Phase {
                event: open.event,
                start: open.hours,
                hours,
                from_level: open.level,
                to_level: close.level,
                insight: close.insight.saturating_sub(open.insight),
                rate: open.rate,
                exp: open.rate * hours,
                bonus: open.bonus,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::Sample;

    fn projection(samples: Vec<Sample>) -> Projection {
        let exp: f64 = samples
            .windows(2)
            .map(|pair| pair[0].rate * (pair[1].hours - pair[0].hours))
            .sum::<f64>()
            + samples.iter().map(|sample| sample.bonus).sum::<f64>();
        Projection {
            hours: samples.last().map_or(0.0, |sample| sample.hours),
            final_level: samples.last().map_or(0, |sample| sample.level),
            total_exp: exp,
            insight_gained: samples.last().map_or(0, |sample| sample.insight),
            samples,
        }
    }

    #[test]
    fn the_phases_account_for_the_projections_hours_and_exp() {
        let run = projection(vec![
            Sample {
                hours: 0.0,
                level: 5,
                insight: 0,
                rate: 100.0,
                event: Event::Start,
                bonus: 0.0,
            },
            Sample {
                hours: 4.0,
                level: 6,
                insight: 1,
                rate: 150.0,
                event: Event::Level,
                bonus: 0.0,
            },
            Sample {
                hours: 10.0,
                level: 6,
                insight: 2,
                rate: 200.0,
                event: Event::Insight,
                bonus: 0.0,
            },
        ]);
        let phases = phases(&run);
        assert_eq!(phases.len(), 2);
        assert_eq!(phases[0].event, Event::Start);
        assert_eq!((phases[0].start, phases[0].hours), (0.0, 4.0));
        assert_eq!((phases[0].from_level, phases[0].to_level), (5, 6));
        assert_eq!(phases[0].exp, 400.0);
        assert_eq!((phases[1].start, phases[1].hours), (4.0, 6.0));
        assert_eq!(phases[1].insight, 1);
        assert_eq!(phases[1].exp, 900.0);

        let hours: f64 = phases.iter().map(|phase| phase.hours).sum();
        let exp: f64 = phases.iter().map(|phase| phase.exp).sum();
        assert_eq!(hours, run.hours);
        assert_eq!(exp, run.total_exp);
        assert_eq!(phases.last().expect("a phase").end(), run.hours);
    }

    #[test]
    fn a_tournament_bonus_is_kept_off_the_clock() {
        let run = projection(vec![
            Sample {
                hours: 0.0,
                level: 5,
                insight: 0,
                rate: 100.0,
                event: Event::Start,
                bonus: 0.0,
            },
            Sample {
                hours: 24.0,
                level: 7,
                insight: 0,
                rate: 100.0,
                event: Event::Tournament,
                bonus: 1200.0,
            },
            Sample {
                hours: 30.0,
                level: 7,
                insight: 0,
                rate: 100.0,
                event: Event::End,
                bonus: 0.0,
            },
        ]);
        let phases = phases(&run);
        assert_eq!(phases.len(), 2);
        assert_eq!(phases[0].hours, 24.0);
        assert_eq!(phases[0].exp, 2400.0);
        assert_eq!(phases[0].bonus, 0.0);
        // The registration injects its extra time at the phase it opens.
        assert_eq!((phases[1].start, phases[1].hours), (24.0, 6.0));
        assert_eq!(phases[1].exp, 600.0);
        assert_eq!(phases[1].bonus, 1200.0);
        assert_eq!(run.total_exp, 4200.0);
    }

    #[test]
    fn a_projection_without_events_has_no_phases() {
        let run = projection(vec![Sample {
            hours: 0.0,
            level: 5,
            insight: 0,
            rate: 100.0,
            event: Event::Start,
            bonus: 0.0,
        }]);
        assert!(phases(&run).is_empty());
    }
}
