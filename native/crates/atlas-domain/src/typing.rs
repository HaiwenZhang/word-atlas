//! Unicode-safe spelling and cumulative typing metrics shared by every platform.

use unicode_normalization::UnicodeNormalization;

/// Normalizes keyboard typography while keeping whitespace positions for character scoring.
pub fn clean(text: &str) -> String {
    text.nfkc()
        .map(|ch| match ch {
            '’' | '‘' => '\'',
            '“' | '”' => '"',
            '‐' | '‑' | '–' | '—' => '-',
            ch if ch.is_whitespace() => ' ',
            ch => ch,
        })
        .collect()
}
/// Normalizes a word or search query, including whitespace and letter case.
pub fn normalize(text: &str) -> String {
    clean(text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
/// Keyboard-equivalent strings, ignoring letter case while preserving whitespace and punctuation.
pub fn same(a: &str, b: &str) -> bool {
    clean(a).to_lowercase() == clean(b).to_lowercase()
}

/// Newly inserted characters and how many match the target at their positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Delta {
    /// Newly inserted or replaced characters. Deletions do not increase this value.
    pub total: u32,
    /// Matching newly inserted characters.
    pub correct: u32,
}
/// Counts a single edit without letting deletion erase prior mistakes.
pub fn delta(previous: &str, next: &str, target: &str) -> Delta {
    let before: Vec<_> = clean(previous).chars().collect();
    let after: Vec<_> = clean(next).chars().collect();
    let expected: Vec<_> = clean(target).chars().collect();
    let prefix = before
        .iter()
        .zip(&after)
        .take_while(|(a, b)| a == b)
        .count();
    let suffix = before[prefix..]
        .iter()
        .rev()
        .zip(after[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let added = &after[prefix..after.len() - suffix];
    let correct = added
        .iter()
        .enumerate()
        .filter(|(i, ch)| {
            expected
                .get(prefix + i)
                .is_some_and(|e| ch.to_lowercase().eq(e.to_lowercase()))
        })
        .count();
    Delta {
        total: added.len() as u32,
        correct: correct as u32,
    }
}
/// Correct Unicode characters at matching positions.
pub fn correct_characters(value: &str, target: &str) -> u32 {
    clean(value)
        .chars()
        .zip(clean(target).chars())
        .filter(|(a, b)| a.to_lowercase().eq(b.to_lowercase()))
        .count() as u32
}
/// Current cumulative typing statistics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Metrics {
    /// Accuracy percentage; an untouched exercise is 100%.
    pub accuracy: u32,
    /// Five-character words per minute, zero until one active second elapsed.
    pub wpm: u32,
}
/// Computes bounded accuracy and WPM without rewarding an idle or zero-duration round.
pub fn metrics(correct: u32, attempted: u32, accepted: u32, elapsed_ms: u64) -> Metrics {
    Metrics {
        accuracy: if attempted == 0 {
            100
        } else {
            (correct.min(attempted) as f64 / attempted as f64 * 100.0).round() as u32
        },
        wpm: if elapsed_ms < 1000 {
            0
        } else {
            (accepted as f64 / 5.0 / (elapsed_ms as f64 / 60_000.0)).round() as u32
        },
    }
}
/// Stable FNV-1a hash using the web application's UTF-16 code units.
pub fn hash_seed(value: &str) -> u32 {
    value.encode_utf16().fold(2_166_136_261, |hash, unit| {
        (hash ^ u32::from(unit)).wrapping_mul(16_777_619)
    })
}
/// The web application's deterministic Fisher–Yates shuffle.
pub fn shuffle<T>(values: &mut [T], seed: u64) {
    let mut state = if seed == 0 { 1 } else { seed as u32 };
    for index in (1..values.len()).rev() {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        values.swap(index, state as usize % (index + 1));
    }
}
