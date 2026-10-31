//! Today's plan, plausible quiz choices and first-attempt-only learning journeys.

use std::collections::BTreeSet;

use crate::{
    Catalog, Entry, Grade, Progress, Settings,
    model::{PendingAnswer, Session, Step, StepKind},
    scheduler,
    typing::{hash_seed, normalize, shuffle},
};

/// A deterministic plan for one local calendar date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// Due words, oldest due first.
    pub due: Vec<String>,
    /// Unseen words from the selected starting level, then lower-level fallback.
    pub fresh: Vec<String>,
    /// Estimated active minutes.
    pub minutes: u32,
}

/// Plans using an explicit local day key; UTC day distance is reserved for FSRS.
pub fn today_plan(
    catalog: &Catalog,
    progress: &Progress,
    settings: &Settings,
    now: i64,
    day: &str,
) -> Plan {
    let mut due: Vec<_> = progress
        .cards
        .iter()
        .filter(|(id, c)| c.due <= now && catalog.entry(id).is_some())
        .collect();
    due.sort_by_key(|(_, c)| c.due);
    let due: Vec<_> = due.into_iter().map(|(id, _)| id.clone()).collect();
    let left = settings
        .daily_new
        .saturating_sub(progress.days.get(day).map_or(0, |d| d.n)) as usize;
    let mut fresh = Vec::with_capacity(left);
    for from in [settings.start_level, 0] {
        for entry in catalog.entries() {
            if fresh.len() >= left {
                break;
            }
            if catalog.level(entry) >= from
                && !progress.cards.contains_key(&entry.id)
                && !fresh.contains(&entry.id)
            {
                fresh.push(entry.id.clone());
            }
        }
    }
    let minutes = (due.len() as f64 * 0.25 + fresh.len() as f64 * 0.9)
        .round()
        .max(1.0) as u32;
    Plan {
        due,
        fresh,
        minutes,
    }
}

fn quiz_kind(entry: &Entry, progress: &Progress, settings: &Settings, salt: i64) -> StepKind {
    let card = progress.cards.get(&entry.id);
    let image = entry.m & 1 != 0;
    let roll = hash_seed(&format!("{}{salt}", entry.id)) % 100;
    let mut kind = if card.is_none_or(|c| c.s < 2.0) {
        if image && roll < 45 {
            StepKind::Picture
        } else if roll < 80 {
            StepKind::Meaning
        } else {
            StepKind::Listen
        }
    } else if settings.spell && card.is_some_and(|c| c.s >= 4.0 || roll < 35) {
        if roll < 75 {
            StepKind::Spell
        } else if image {
            StepKind::Picture
        } else {
            StepKind::Listen
        }
    } else if roll < 50 {
        StepKind::Listen
    } else if image {
        StepKind::Picture
    } else {
        StepKind::Meaning
    };
    let audio_bit = if settings.accent == "us" { 4 } else { 2 };
    if kind == StepKind::Listen && entry.m & audio_bit == 0 {
        kind = StepKind::Meaning;
    }
    kind
}

/// Selects three plausible but semantically distinct distractors using stable IDs.
pub fn distractors(catalog: &Catalog, entry: &Entry, kind: StepKind, seed: i64) -> Vec<String> {
    let target = normalize(&entry.w);
    let meaning = entry.senses.first().map_or("", |s| s.zh.as_str());
    let core = meaning.split(['；', ';', '，', ',']).next().unwrap_or("");
    let position = catalog.position(&entry.id).unwrap_or(0);
    let mut scored: Vec<_> = catalog.entries()
        [position.saturating_sub(400)..(position + 400).min(catalog.entries().len())]
        .iter()
        .filter(|word| {
            word.id != entry.id
                && normalize(&word.w) != target
                && (core.is_empty() || !word.senses.iter().any(|s| s.zh.contains(core)))
        })
        .map(|word| {
            let mut score = (hash_seed(&format!("{}{seed}", word.id)) % 100) as i32;
            if [StepKind::Listen, StepKind::Picture].contains(&kind) {
                if word
                    .w
                    .to_lowercase()
                    .starts_with(target.chars().next().unwrap_or('\0'))
                {
                    score += 120;
                }
                score += (60 - (word.w.len() as i32 - target.len() as i32).abs() * 20).max(0);
            } else {
                if word.cefr == entry.cefr {
                    score += 40;
                }
                if word.senses.first().map(|s| &s.pos) == entry.senses.first().map(|s| &s.pos) {
                    score += 60;
                }
            }
            (score, word)
        })
        .collect();
    scored.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    let mut seen = BTreeSet::new();
    let mut picks = vec![entry.id.clone()];
    for (_, word) in scored {
        let key = if kind == StepKind::Meaning {
            word.senses
                .first()
                .map_or_else(String::new, |s| s.zh.clone())
        } else {
            normalize(&word.w)
        };
        if seen.insert(key) {
            picks.push(word.id.clone());
        }
        if picks.len() == 4 {
            break;
        }
    }
    shuffle(&mut picks, seed as u64);
    picks
}

fn quiz(
    catalog: &Catalog,
    progress: &Progress,
    settings: &Settings,
    id: &str,
    seed: i64,
) -> Option<Step> {
    let entry = catalog.entry(id)?;
    let kind = quiz_kind(entry, progress, settings, seed);
    Some(Step {
        id: id.into(),
        kind,
        retry: false,
        options: if kind == StepKind::Spell {
            None
        } else {
            Some(distractors(catalog, entry, kind, seed))
        },
    })
}

/// Builds a journey: at most 120 due words, then five introductions followed by five recalls.
pub fn build(
    catalog: &Catalog,
    progress: &Progress,
    settings: &Settings,
    now: i64,
    day: &str,
    review_only: bool,
    ids: Option<&[String]>,
) -> Session {
    let plan = today_plan(catalog, progress, settings, now, day);
    let review_ids: Vec<_> = ids.map_or_else(
        || plan.due.into_iter().take(120).collect(),
        |ids| {
            ids.iter()
                .filter(|id| progress.cards.contains_key(*id) && catalog.entry(id).is_some())
                .cloned()
                .collect()
        },
    );
    let new_ids: Vec<_> = ids.map_or_else(
        || if review_only { vec![] } else { plan.fresh },
        |ids| {
            ids.iter()
                .filter(|id| !progress.cards.contains_key(*id) && catalog.entry(id).is_some())
                .cloned()
                .collect()
        },
    );
    let mut steps = Vec::new();
    for (i, id) in review_ids.iter().enumerate() {
        if let Some(step) = quiz(catalog, progress, settings, id, now + i as i64) {
            steps.push(step);
        }
    }
    for (group_index, group) in new_ids.chunks(5).enumerate() {
        steps.extend(group.iter().map(|id| Step {
            id: id.clone(),
            kind: StepKind::Intro,
            retry: false,
            options: None,
        }));
        let mut order = group.to_vec();
        shuffle(&mut order, (now + group_index as i64 * 5) as u64);
        for (i, id) in order.iter().enumerate() {
            if let Some(step) = quiz(
                catalog,
                progress,
                settings,
                id,
                now + 1000 + group_index as i64 * 5 + i as i64,
            ) {
                steps.push(step);
            }
        }
    }
    Session {
        id: format!("s{now}"),
        started: now,
        steps,
        index: 0,
        graded: Default::default(),
        new_ids,
        review_ids,
        pending: None,
        finished: None,
    }
}

/// Automatic grade for a correct/incorrect answer and the time/hints it required.
pub fn grade_for(kind: StepKind, correct: bool, milliseconds: u64, hinted: bool) -> Grade {
    if !correct {
        Grade::Again
    } else if hinted {
        Grade::Hard
    } else {
        let slow = if kind == StepKind::Spell {
            20_000
        } else {
            9_000
        };
        if milliseconds > slow {
            Grade::Hard
        } else if kind == StepKind::Spell && milliseconds < 6_000 {
            Grade::Easy
        } else {
            Grade::Good
        }
    }
}

/// An answer or continuation that cannot be applied safely.
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    /// There is no active question.
    #[error("没有正在进行的题目。")]
    NoQuestion,
    /// An answer is already awaiting continuation.
    #[error("这一题已提交，请继续。")]
    AlreadyAnswered,
    /// A vocabulary entry no longer exists.
    #[error("找不到词条：{0}")]
    UnknownWord(String),
    /// Memory scheduling failed.
    #[error(transparent)]
    Scheduler(#[from] scheduler::SchedulerError),
}

/// One submitted answer together with its response time and local-day context.
#[derive(Debug, Clone, Copy)]
pub struct Submission<'a> {
    /// Submitted word or option ID.
    pub value: &'a str,
    /// Active response time.
    pub milliseconds: u64,
    /// At least one hint was requested.
    pub hinted: bool,
    /// Rating time in Unix milliseconds.
    pub now: i64,
    /// The user's local calendar date.
    pub day: &'a str,
}

/// Applies a first submission transactionally; practice retries never update memory or accuracy twice.
pub fn answer(
    catalog: &Catalog,
    progress: &mut Progress,
    settings: &Settings,
    submission: Submission<'_>,
) -> Result<bool, SessionError> {
    let Submission {
        value,
        milliseconds,
        hinted,
        now,
        day,
    } = submission;
    let session = progress.session.as_ref().ok_or(SessionError::NoQuestion)?;
    if session.pending.is_some() {
        return Err(SessionError::AlreadyAnswered);
    }
    let step = session
        .steps
        .get(session.index)
        .ok_or(SessionError::NoQuestion)?
        .clone();
    if step.kind == StepKind::Intro || session.finished.is_some() {
        return Err(SessionError::NoQuestion);
    }
    let entry = catalog
        .entry(&step.id)
        .ok_or_else(|| SessionError::UnknownWord(step.id.clone()))?;
    let correct = if step.kind == StepKind::Spell {
        normalize(value) == normalize(&entry.w)
    } else {
        if !step
            .options
            .as_ref()
            .is_some_and(|options| options.iter().any(|id| id == value))
        {
            return Err(SessionError::NoQuestion);
        }
        value == step.id
    };
    let grade = grade_for(step.kind, correct, milliseconds, hinted);
    let first = !step.retry && !session.graded.contains_key(&step.id);
    let scheduled = if first {
        Some(scheduler::review(
            progress.cards.get(&step.id),
            grade,
            now,
            settings.retention,
        )?)
    } else {
        None
    };
    let is_new = session.new_ids.contains(&step.id);
    let Some(session) = progress.session.as_mut() else {
        return Err(SessionError::NoQuestion);
    };
    session.pending = Some(PendingAnswer {
        correct,
        value: value.into(),
    });
    if let Some(card) = scheduled {
        progress.cards.insert(step.id.clone(), card);
        session.graded.insert(step.id.clone(), grade);
        let stats = progress.days.entry(day.into()).or_default();
        stats.r += 1;
        stats.c += u32::from(correct);
        stats.n += u32::from(is_new);
        if !correct {
            progress.mistakes.retain(|id| id != &step.id);
            progress.mistakes.insert(0, step.id.clone());
            progress.mistakes.truncate(300);
        }
    }
    if !correct
        && session
            .steps
            .iter()
            .filter(|s| s.id == step.id && s.retry)
            .count()
            < 2
    {
        let kind = if step.kind == StepKind::Meaning {
            if entry.m & 1 != 0 {
                StepKind::Picture
            } else {
                StepKind::Listen
            }
        } else {
            StepKind::Meaning
        };
        let at = (session.index + 4).min(session.steps.len());
        session.steps.insert(
            at,
            Step {
                id: step.id,
                kind,
                retry: true,
                options: Some(distractors(catalog, entry, kind, now)),
            },
        );
    }
    Ok(correct)
}

/// Advances an introduction or an answered quiz; idle time is capped at 90 seconds per step.
pub fn advance(
    progress: &mut Progress,
    seconds: f64,
    now: i64,
    day: &str,
) -> Result<(), SessionError> {
    let session = progress.session.as_mut().ok_or(SessionError::NoQuestion)?;
    let step = session
        .steps
        .get(session.index)
        .ok_or(SessionError::NoQuestion)?;
    if step.kind != StepKind::Intro && session.pending.is_none() {
        return Err(SessionError::NoQuestion);
    }
    session.index += 1;
    session.pending = None;
    if session.index >= session.steps.len() {
        session.finished = Some(now);
    }
    let seconds = if seconds.is_finite() {
        seconds.clamp(0.0, 90.0).round() as u32
    } else {
        0
    };
    progress.days.entry(day.into()).or_default().t += seconds;
    Ok(())
}

/// First-attempt accuracy for a session, independent of practice retries.
pub fn accuracy(session: &Session) -> u32 {
    if session.graded.is_empty() {
        0
    } else {
        (session
            .graded
            .values()
            .filter(|g| **g != Grade::Again)
            .count() as f64
            / session.graded.len() as f64
            * 100.0)
            .round() as u32
    }
}
