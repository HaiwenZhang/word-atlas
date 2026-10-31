//! FSRS-6 with the same weights, rounding, short-term steps and seeded fuzz as ts-fsrs 5.4.2.
//!
//! Ported from the MIT-licensed ts-fsrs; see native/LICENSES/ts-fsrs.txt.

use crate::model::{Card, Grade};

const DAY: i64 = 86_400_000;
const MAX_INTERVAL: f64 = 36_500.0;
const W: [f64; 21] = [
    0.212, 1.2931, 2.3065, 8.2956, 6.4133, 0.8334, 3.0194, 0.001, 1.8722, 0.1666, 0.796, 1.4835,
    0.0614, 0.2629, 1.6483, 0.6014, 1.8729, 0.5425, 0.0912, 0.0658, 0.1542,
];

/// A review cannot safely be applied to this memory or time.
#[derive(Debug, thiserror::Error)]
pub enum SchedulerError {
    /// Invalid imported card.
    #[error("记忆数据无效，请从备份恢复。")]
    InvalidCard,
    /// System time predates the previous review.
    #[error("系统时间早于上次学习，请检查设备时间。")]
    Clock,
    /// Unsupported target retention.
    #[error("目标记忆率必须为 70%–97%。")]
    Retention,
}

fn round8(value: f64) -> f64 {
    (value * 100_000_000.0).round() / 100_000_000.0
}
fn clamp(value: f64, min: f64, max: f64) -> f64 {
    value.max(min).min(max)
}
fn initial_difficulty(grade: Grade) -> f64 {
    round8(W[4] - ((grade as u8 as f64 - 1.0) * W[5]).exp() + 1.0)
}
fn factor() -> f64 {
    round8(((-W[20]).recip() * 0.9_f64.ln()).exp() - 1.0)
}

/// Probability of recall using UTC calendar-day distance, as the web scheduler does.
pub fn retrievability(card: &Card, now: i64) -> f64 {
    let elapsed = card
        .last
        .map_or(0, |last| now.div_euclid(DAY) - last.div_euclid(DAY))
        .max(0);
    if card.s <= 0.0 {
        return 0.0;
    }
    round8((1.0 + factor() * elapsed as f64 / card.s).powf(-W[20]))
}

fn next_memory(previous: &Card, grade: Grade, days: u32) -> (f64, f64) {
    if previous.s == 0.0 && previous.d == 0.0 {
        return (
            W[grade as usize - 1].max(0.1),
            clamp(initial_difficulty(grade), 1.0, 10.0),
        );
    }
    let s = previous.s;
    let d = previous.d;
    let g = grade as u8 as f64;
    let r = round8((1.0 + factor() * days as f64 / s).powf(-W[20]));
    let stability = if days == 0 {
        let increase = s.powf(-W[19]) * (W[17] * (g - 3.0 + W[18])).exp();
        round8(clamp(
            s * if grade == Grade::Again {
                increase
            } else {
                increase.max(1.0)
            },
            0.001,
            MAX_INTERVAL,
        ))
    } else if grade == Grade::Again {
        let after_failure = round8(clamp(
            W[11] * d.powf(-W[12]) * ((s + 1.0).powf(W[13]) - 1.0) * ((1.0 - r) * W[14]).exp(),
            0.001,
            MAX_INTERVAL,
        ));
        clamp(round8(s / (W[17] * W[18]).exp()), 0.001, after_failure)
    } else {
        let hard = if grade == Grade::Hard { W[15] } else { 1.0 };
        let easy = if grade == Grade::Easy { W[16] } else { 1.0 };
        round8(clamp(
            s * (1.0
                + W[8].exp()
                    * (11.0 - d)
                    * s.powf(-W[9])
                    * (((1.0 - r) * W[10]).exp() - 1.0)
                    * hard
                    * easy),
            0.001,
            MAX_INTERVAL,
        ))
    };
    let delta = round8(-W[6] * (g - 3.0) * (10.0 - d) / 9.0);
    let difficulty = clamp(
        round8(W[7] * initial_difficulty(Grade::Easy) + (1.0 - W[7]) * (d + delta)),
        1.0,
        10.0,
    );
    (stability, difficulty)
}

/// The next state for every grade; preview and actual review share the same computation.
pub fn preview(
    previous: Option<&Card>,
    now: i64,
    retention: f64,
) -> Result<[Card; 4], SchedulerError> {
    if !(0.7..=0.97).contains(&retention) {
        return Err(SchedulerError::Retention);
    }
    let empty = Card::new(now);
    let previous = previous.unwrap_or(&empty);
    if !previous.is_valid() {
        return Err(SchedulerError::InvalidCard);
    }
    if now < 0 || previous.last.is_some_and(|last| now < last) {
        return Err(SchedulerError::Clock);
    }
    let elapsed = if previous.st == 0 {
        0
    } else {
        previous
            .last
            .map_or(0, |last| now.div_euclid(DAY) - last.div_euclid(DAY)) as u32
    };
    let repetitions = previous
        .reps
        .checked_add(1)
        .ok_or(SchedulerError::InvalidCard)?;
    let seed = format!("{now}_{repetitions}_{}", previous.d * previous.s);
    let random = alea_first(&seed);
    let modifier = round8((retention.powf((-W[20]).recip()) - 1.0) / factor());
    let next_interval = |stability: f64| -> u32 {
        let interval = clamp((stability * modifier).round(), 1.0, MAX_INTERVAL);
        if interval < 2.5 {
            return interval.round() as u32;
        }
        let delta = 1.0
            + 0.15 * (interval.min(7.0) - 2.5).max(0.0)
            + 0.1 * (interval.min(20.0) - 7.0).max(0.0)
            + 0.05 * (interval - 20.0).max(0.0);
        let mut min = (interval - delta).round().max(2.0);
        let max = (interval + delta).round().min(MAX_INTERVAL);
        if interval > elapsed as f64 {
            min = min.max(elapsed as f64 + 1.0);
        }
        min = min.min(max);
        (random * (max - min + 1.0) + min).floor() as u32
    };
    let mut cards = Grade::ALL.map(|grade| {
        let (s, d) = next_memory(previous, grade, elapsed);
        Card {
            s,
            d,
            el: elapsed,
            reps: repetitions,
            last: Some(now),
            ..previous.clone()
        }
    });
    if previous.st == 2 {
        let hard = next_interval(cards[1].s).min(next_interval(cards[2].s));
        let good = next_interval(cards[2].s).max(hard + 1);
        let easy = next_interval(cards[3].s).max(good + 1);
        for (card, interval) in cards[1..].iter_mut().zip([hard, good, easy]) {
            card.sd = interval;
            card.due = now + i64::from(interval) * DAY;
            card.st = 2;
            card.ls = 0;
        }
        cards[0].st = 3;
        cards[0].sd = 0;
        cards[0].ls = 0;
        cards[0].due = now + 600_000;
        cards[0].lapses = previous
            .lapses
            .checked_add(1)
            .ok_or(SchedulerError::InvalidCard)?;
    } else {
        let relearning = previous.st == 3;
        let steps = if relearning {
            &[10_u32][..]
        } else {
            &[1_u32, 10][..]
        };
        for (card, grade) in cards.iter_mut().zip(Grade::ALL) {
            let (minutes, step) = if previous.ls as usize >= steps.len() {
                (0, 0)
            } else {
                match grade {
                    Grade::Again => (steps[0], 0),
                    Grade::Hard => (
                        if steps.len() == 1 {
                            (steps[0] as f64 * 1.5).round() as u32
                        } else {
                            ((steps[0] + steps[1]) as f64 / 2.0).round() as u32
                        },
                        previous.ls,
                    ),
                    Grade::Good => (
                        steps.get(previous.ls as usize + 1).copied().unwrap_or(0),
                        previous.ls + 1,
                    ),
                    Grade::Easy => (0, 0),
                }
            };
            if minutes > 0 {
                card.ls = step;
                card.sd = 0;
                card.st = if relearning { 3 } else { 1 };
                card.due = now + i64::from(minutes) * 60_000;
            } else {
                card.ls = 0;
                card.sd = next_interval(card.s);
                card.st = 2;
                card.due = now + i64::from(card.sd) * DAY;
            }
        }
    }
    Ok(cards)
}

/// Applies one rating without mutating the previous card.
pub fn review(
    previous: Option<&Card>,
    grade: Grade,
    now: i64,
    retention: f64,
) -> Result<Card, SchedulerError> {
    Ok(preview(previous, now, retention)?[grade as usize - 1].clone())
}

// The web implementation resets Alea to the same seed for every candidate interval.
fn alea_first(seed: &str) -> f64 {
    const SCALE: f64 = 2.328_306_436_538_696_3e-10;
    let mut n = 4_022_871_197.0;
    let mut mash = |data: &str| -> f64 {
        for unit in data.encode_utf16() {
            n += f64::from(unit);
            let mut h = 0.025_196_032_824_169_38 * n;
            n = f64::from(h as u32);
            h -= n;
            h *= n;
            n = f64::from(h as u32);
            h -= n;
            n += h * 4_294_967_296.0;
        }
        f64::from(n as u64 as u32) * SCALE
    };
    let mut s0 = mash(" ");
    let _s1 = mash(" ");
    let _s2 = mash(" ");
    s0 -= mash(seed);
    if s0 < 0.0 {
        s0 += 1.0;
    }
    // s1 and s2 do not contribute until the second and third calls.
    let t = 2_091_639.0 * s0 + SCALE;
    t - t.floor()
}

/// Compact Chinese interval label for rating controls and word details.
pub fn format_interval(milliseconds: i64) -> String {
    let minutes = (milliseconds as f64 / 60_000.0).round() as i64;
    if minutes < 1 {
        "<1分".into()
    } else if minutes < 60 {
        format!("{minutes}分")
    } else {
        let hours = (minutes as f64 / 60.0).round() as i64;
        if hours < 24 {
            format!("{hours}小时")
        } else {
            let days = (hours as f64 / 24.0).round() as i64;
            if days < 31 {
                format!("{days}天")
            } else {
                let months = (days as f64 / 30.0).round() as i64;
                if months < 12 {
                    format!("{months}个月")
                } else {
                    format!("{:.1}年", days as f64 / 365.0)
                }
            }
        }
    }
}
