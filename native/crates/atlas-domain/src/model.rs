//! Persistent models compatible with version 2 of the existing web application.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Four FSRS answer grades, serialized as the web application's integers 1 through 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Grade {
    /// Failed recall.
    Again = 1,
    /// Recalled with difficulty or a hint.
    Hard = 2,
    /// Successful recall.
    Good = 3,
    /// Effortless recall.
    Easy = 4,
}
impl Grade {
    /// Every grade in ascending order.
    pub const ALL: [Self; 4] = [Self::Again, Self::Hard, Self::Good, Self::Easy];
    /// Human-readable Chinese label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Again => "忘记",
            Self::Hard => "困难",
            Self::Good => "记得",
            Self::Easy => "轻松",
        }
    }
}
impl Serialize for Grade {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(*self as u8)
    }
}
impl<'de> Deserialize<'de> for Grade {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match u8::deserialize(deserializer)? {
            1 => Ok(Self::Again),
            2 => Ok(Self::Hard),
            3 => Ok(Self::Good),
            4 => Ok(Self::Easy),
            _ => Err(serde::de::Error::custom("评分必须是 1–4")),
        }
    }
}

/// Serialized FSRS memory; all times are Unix milliseconds and stability is in days.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Card {
    /// Next review time.
    pub due: i64,
    /// Stability.
    pub s: f64,
    /// Difficulty.
    pub d: f64,
    /// Elapsed UTC calendar days at the last rating.
    pub el: u32,
    /// Scheduled interval in days.
    pub sd: u32,
    /// Current short-term learning step.
    pub ls: u32,
    /// Number of first-attempt reviews.
    pub reps: u32,
    /// Number of lapses after review graduation.
    pub lapses: u32,
    /// New=0, Learning=1, Review=2, Relearning=3.
    pub st: u8,
    /// Last rating time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last: Option<i64>,
    /// First encountered time.
    pub seen: i64,
}
impl Card {
    /// A new memory card, before the first rating.
    pub fn new(now: i64) -> Self {
        Self {
            due: now,
            s: 0.0,
            d: 0.0,
            el: 0,
            sd: 0,
            ls: 0,
            reps: 0,
            lapses: 0,
            st: 0,
            last: None,
            seen: now,
        }
    }
    /// Long-term retention threshold shared with the existing web application.
    pub fn is_mastered(&self) -> bool {
        self.st == 2 && self.s >= 21.0
    }
    /// Rejects invalid imported or persisted memory state before it reaches the scheduler.
    pub fn is_valid(&self) -> bool {
        const MAX_TIME: i64 = 8_640_000_000_000_000;
        (0..=MAX_TIME).contains(&self.due)
            && (0..=MAX_TIME).contains(&self.seen)
            && self.last.is_none_or(|t| (0..=MAX_TIME).contains(&t))
            && self.s.is_finite()
            && self.s >= 0.0
            && self.d.is_finite()
            && (0.0..=11.0).contains(&self.d)
            && self.st <= 3
            && (self.st == 0 || (self.s >= 0.001 && self.d >= 1.0 && self.last.is_some()))
    }
}

/// Quiz format, including the first encounter with a new word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StepKind {
    /// Introduce a new word.
    Intro,
    /// Pick the meaning of a written word.
    Meaning,
    /// Pick the word from its illustration and sentence.
    Picture,
    /// Identify a word by listening.
    Listen,
    /// Type the word from memory.
    Spell,
}

/// One saved session step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Step {
    /// Vocabulary ID.
    pub id: String,
    /// Question format.
    pub kind: StepKind,
    /// Practice-only retry, which is never counted twice.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub retry: bool,
    /// Stable option IDs for multiple-choice questions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<String>>,
}

/// A submitted answer that is still visible when the app is reopened.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PendingAnswer {
    /// Whether the first submission was correct.
    pub correct: bool,
    /// Submitted answer or option ID.
    pub value: String,
}

/// A resumable learning journey; only `graded` entries affect memory and statistics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    /// Unique session identifier.
    pub id: String,
    /// Start time.
    pub started: i64,
    /// Ordered steps, including inserted practice retries.
    pub steps: Vec<Step>,
    /// Current step index.
    pub index: usize,
    /// First-attempt ratings by ID.
    pub graded: BTreeMap<String, Grade>,
    /// New word IDs.
    pub new_ids: Vec<String>,
    /// Review word IDs.
    pub review_ids: Vec<String>,
    /// Answer awaiting continuation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending: Option<PendingAnswer>,
    /// Completion time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished: Option<i64>,
}

/// Daily first-attempt statistics, keyed by the user's local calendar date.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct DayStat {
    /// New words rated.
    pub n: u32,
    /// First-attempt reviews.
    pub r: u32,
    /// Correct first attempts.
    pub c: u32,
    /// Active learning seconds.
    pub t: u32,
}

/// A completed typing exercise.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypingRound {
    /// Completion time in Unix milliseconds.
    pub at: i64,
    /// `word`, `sentence` or `dictation`.
    pub mode: String,
    /// Accepted items.
    pub items: u32,
    /// Words per minute, based on five characters per word.
    pub wpm: u32,
    /// Cumulative character accuracy percentage.
    pub accuracy: u32,
}

/// All user learning data. Schema names intentionally preserve web backup compatibility.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// Persistent schema, currently 2.
    pub version: u8,
    /// Canonical dataset identifier.
    pub dataset_id: String,
    /// Per-word FSRS state.
    pub cards: BTreeMap<String, Card>,
    /// Starred words.
    pub bookmarks: Vec<String>,
    /// Local daily learning statistics.
    pub days: BTreeMap<String, DayStat>,
    /// Most recent 500 typing rounds.
    pub typing: Vec<TypingRound>,
    /// Active or completed journey awaiting its summary.
    pub session: Option<Session>,
    /// Most recent 300 mistakes, newest first.
    pub mistakes: Vec<String>,
}
impl Progress {
    /// Creates an empty dataset-bound progress record.
    pub fn new(dataset: &str) -> Self {
        Self {
            version: 2,
            dataset_id: dataset.into(),
            cards: BTreeMap::new(),
            bookmarks: vec![],
            days: BTreeMap::new(),
            typing: vec![],
            session: None,
            mistakes: vec![],
        }
    }
    /// Adds or removes a bookmark without changing its learning state.
    pub fn toggle_bookmark(&mut self, id: &str) {
        if self.bookmarks.iter().any(|b| b == id) {
            self.bookmarks.retain(|b| b != id);
        } else {
            self.bookmarks.insert(0, id.into());
        }
    }
}

/// User preferences, with web-compatible property names.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// `uk` or `us`.
    pub accent: String,
    /// Play the current word automatically.
    pub autoplay: bool,
    /// Daily new-word target.
    pub daily_new: u32,
    /// Desired recall probability, 0.70–0.97.
    pub retention: f64,
    /// `system`, `light` or `dark`.
    pub theme: String,
    /// Interface sounds enabled.
    pub sfx: bool,
    /// Include spelling questions.
    pub spell: bool,
    /// Starting CEFR level index.
    pub start_level: usize,
    /// Initial setup completed.
    pub onboarded: bool,
    /// Canonical catalog kind, `open`.
    pub dataset: String,
    /// Background ambience identifier or `off`.
    pub ambience: String,
    /// Background volume, 0–1.
    pub ambience_volume: f64,
    /// Welcome animation enabled.
    pub splash: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            accent: "uk".into(),
            autoplay: true,
            daily_new: 10,
            retention: 0.9,
            theme: "system".into(),
            sfx: true,
            spell: true,
            start_level: 0,
            onboarded: false,
            dataset: "open".into(),
            ambience: "stars".into(),
            ambience_volume: 0.45,
            splash: true,
        }
    }
}
impl Settings {
    /// Rejects unsupported preferences before committing a snapshot.
    pub fn is_valid(&self) -> bool {
        ["uk", "us"].contains(&self.accent.as_str())
            && self.daily_new <= 100
            && self.start_level <= 5
            && self.retention.is_finite()
            && (0.7..=0.97).contains(&self.retention)
            && ["system", "light", "dark"].contains(&self.theme.as_str())
            && self.dataset == "open"
            && [
                "off", "stars", "rain", "sea", "forest", "fire", "voyage", "cafe",
            ]
            .contains(&self.ambience.as_str())
            && self.ambience_volume.is_finite()
            && (0.0..=1.0).contains(&self.ambience_volume)
    }
}
