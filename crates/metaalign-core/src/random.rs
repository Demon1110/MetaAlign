//! Random timestamp generation.

use chrono::{DateTime, TimeDelta, Utc};
use rand::RngExt;

use crate::model::{FieldMode, SequenceMode, TimeField, TimePatch, TimeRange, TimeRule};

/// Milliseconds between two instants.
fn millis_between(a: DateTime<Utc>, b: DateTime<Utc>) -> i64 {
    b.timestamp_millis() - a.timestamp_millis()
}

fn add_millis(base: DateTime<Utc>, delta: i64) -> DateTime<Utc> {
    let value = base + TimeDelta::milliseconds(delta);
    // Clamp to a range Windows can actually store (1601-01-01 .. 30828).
    if value.timestamp() < -11_644_473_600 {
        DateTime::from_timestamp(-11_644_473_600, 0).unwrap_or(base)
    } else {
        value
    }
}

/// Random instant inside `[start, end]`, inclusive on both ends.
pub fn random_instant(range: &TimeRange, rng: &mut impl RngExt) -> DateTime<Utc> {
    let (start, end) = range.normalized();
    let span = millis_between(start, end).max(1);
    let offset = rng.random_range(0..=span);
    add_millis(start, offset)
}

/// Random instant in `[start, end]`, preferring the last `min_span` milliseconds
/// so the result stays close to `end` while remaining inside the window.
fn random_near(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    min_span: i64,
    rng: &mut impl RngExt,
) -> DateTime<Utc> {
    let span = millis_between(start, end);
    if span <= 0 {
        return start;
    }
    let low = min_span.clamp(1, span);
    add_millis(start, rng.random_range(low..=span))
}

fn clamp_instant(value: DateTime<Utc>, low: DateTime<Utc>, high: DateTime<Utc>) -> DateTime<Utc> {
    if value < low {
        low
    } else if value > high {
        high
    } else {
        value
    }
}

/// Anchor instant for a file inside the batch, honouring the sequence mode.
fn anchor_instant(
    rule: &TimeRule,
    index: usize,
    total: usize,
    rng: &mut impl RngExt,
) -> DateTime<Utc> {
    let (start, end) = rule.range.normalized();
    let span = millis_between(start, end).max(1);
    match rule.sequence {
        SequenceMode::Independent => random_instant(&rule.range, rng),
        SequenceMode::Ascending | SequenceMode::Descending => {
            let total = total.max(1);
            let fraction = (index as f64 + 0.5) / total as f64;
            let fraction = if rule.sequence == SequenceMode::Ascending {
                fraction
            } else {
                1.0 - fraction
            };
            // A little jitter keeps the series from looking like a ruler.
            let jitter_span = (span as f64 * 0.02 * fraction.min(0.9)) as i64;
            let jitter = if jitter_span > 0 {
                rng.random_range(-jitter_span..=jitter_span)
            } else {
                0
            };
            add_millis(add_millis(start, (span as f64 * fraction) as i64), jitter)
        }
    }
}

fn resolve(field: &TimeField, random_value: Option<DateTime<Utc>>) -> Option<DateTime<Utc>> {
    match field.mode {
        FieldMode::Keep => None,
        FieldMode::Fixed => Some(field.fixed),
        FieldMode::Random => random_value,
    }
}

/// Generate the timestamp patch for one file.
///
/// `index` is the position of the file in the batch and `total` the batch size;
/// both are only used by the ordered sequence modes.
pub fn generate_times(
    rule: &TimeRule,
    index: usize,
    total: usize,
    rng: &mut impl RngExt,
) -> TimePatch {
    let (range_start, range_end) = rule.range.normalized();
    let anchor = clamp_instant(
        anchor_instant(rule, index, total, rng),
        range_start,
        range_end,
    );

    // Created sits somewhere between the window start and the anchor, accessed
    // between the anchor and the window end: this keeps every value inside the
    // requested window while still producing a plausible creation → access story.
    let random_created = random_near(range_start, anchor, 60_000, rng);
    let random_modified = anchor;
    let random_accessed = random_near(anchor, range_end, 60_000, rng);

    let mut created = resolve(&rule.created, Some(random_created));
    let mut modified = resolve(&rule.modified, Some(random_modified));
    let mut accessed = resolve(&rule.accessed, Some(random_accessed));

    if rule.respect_order {
        let all_random = rule.created.mode == FieldMode::Random
            && rule.modified.mode == FieldMode::Random
            && rule.accessed.mode == FieldMode::Random;
        if all_random {
            if let (Some(c), Some(m), Some(a)) = (created, modified, accessed) {
                let mut values = [c, m, a];
                values.sort();
                created = Some(values[0]);
                modified = Some(values[1]);
                accessed = Some(values[2]);
            }
        } else {
            // With a mix of fixed and random values we nudge instead of reordering.
            if let (Some(c), Some(m)) = (created, modified)
                && c >= m
                && rule.created.mode == FieldMode::Random
            {
                created = Some(clamp_instant(
                    m - TimeDelta::seconds(1),
                    range_start,
                    range_end,
                ));
            }
            if let (Some(m), Some(a)) = (modified, accessed)
                && a <= m
                && rule.accessed.mode == FieldMode::Random
            {
                accessed = Some(clamp_instant(
                    m + TimeDelta::seconds(1),
                    range_start,
                    range_end,
                ));
            }
        }
    }

    TimePatch {
        created: created.map(|value| clamp_instant(value, range_start, range_end)),
        modified: modified.map(|value| clamp_instant(value, range_start, range_end)),
        accessed: accessed.map(|value| clamp_instant(value, range_start, range_end)),
    }
}

/// A handful of generated values for the live preview in the UI.
pub fn sample_times(rule: &TimeRule, count: usize) -> Vec<TimePatch> {
    let mut rng = rand::rng();
    (0..count)
        .map(|index| generate_times(rule, index, count.max(1), &mut rng))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule_with_range(start_ms: i64, end_ms: i64) -> TimeRule {
        TimeRule {
            range: TimeRange {
                start: DateTime::from_timestamp_millis(start_ms).unwrap(),
                end: DateTime::from_timestamp_millis(end_ms).unwrap(),
            },
            ..Default::default()
        }
    }

    #[test]
    fn generated_values_stay_inside_the_window() {
        let start = 1_600_000_000_000i64;
        let end = 1_640_000_000_000i64;
        let rule = rule_with_range(start, end);
        let mut rng = rand::rng();
        for _ in 0..200 {
            let patch = generate_times(&rule, 0, 1, &mut rng);
            for value in [patch.created, patch.modified, patch.accessed]
                .into_iter()
                .flatten()
            {
                assert!(value.timestamp_millis() >= start, "{value:?} < start");
                assert!(value.timestamp_millis() <= end, "{value:?} > end");
            }
        }
    }

    #[test]
    fn respect_order_keeps_chronology() {
        let rule = rule_with_range(1_600_000_000_000, 1_610_000_000_000);
        let mut rng = rand::rng();
        for _ in 0..200 {
            let patch = generate_times(&rule, 0, 1, &mut rng);
            let (c, m, a) = (
                patch.created.unwrap(),
                patch.modified.unwrap(),
                patch.accessed.unwrap(),
            );
            assert!(c <= m && m <= a, "{c:?} {m:?} {a:?}");
        }
    }

    #[test]
    fn ordered_sequences_are_monotonic() {
        for mode in [SequenceMode::Ascending, SequenceMode::Descending] {
            let mut rule = rule_with_range(1_600_000_000_000, 1_640_000_000_000);
            rule.sequence = mode;
            let mut rng = rand::rng();
            let anchors: Vec<_> = (0..10)
                .map(|index| generate_times(&rule, index, 10, &mut rng).modified.unwrap())
                .collect();
            let sorted_asc = anchors.windows(2).all(|w| w[0] <= w[1]);
            assert_eq!(mode == SequenceMode::Ascending, sorted_asc, "{anchors:?}");
        }
    }

    #[test]
    fn keep_mode_produces_nothing() {
        let rule = TimeRule {
            created: TimeField::keep(),
            modified: TimeField::keep(),
            accessed: TimeField::keep(),
            ..Default::default()
        };
        let patch = generate_times(&rule, 0, 1, &mut rand::rng());
        assert!(patch.is_empty());
    }
}
