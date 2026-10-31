//! Cross-language verification against the existing web scheduler and real vocabulary.
#![expect(
    clippy::unwrap_used,
    reason = "Malformed fixtures and failed assertions should fail tests loudly."
)]

use atlas_domain::{
    Card, Catalog, Progress, Settings, model::StepKind, scheduler, session, typing,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Case {
    retention: f64,
    now: i64,
    previous: Option<Card>,
    expected: [Card; 4],
}

#[test]
fn all_fsrs_states_should_match_the_locked_web_scheduler() {
    let fixtures: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/fsrs-web-v5.4.2.json")).unwrap();
    for (case_index, case) in fixtures.iter().enumerate() {
        let actual = scheduler::preview(case.previous.as_ref(), case.now, case.retention).unwrap();
        for (grade, (actual, expected)) in actual.iter().zip(&case.expected).enumerate() {
            assert!(
                (actual.s - expected.s).abs() < 0.00000002,
                "case {case_index} grade {grade} stability: {actual:?} != {expected:?}"
            );
            assert!(
                (actual.d - expected.d).abs() < 0.00000002,
                "case {case_index} grade {grade} difficulty: {actual:?} != {expected:?}"
            );
            assert_eq!(
                (
                    actual.due,
                    actual.el,
                    actual.sd,
                    actual.ls,
                    actual.reps,
                    actual.lapses,
                    actual.st,
                    actual.last,
                    actual.seen
                ),
                (
                    expected.due,
                    expected.el,
                    expected.sd,
                    expected.ls,
                    expected.reps,
                    expected.lapses,
                    expected.st,
                    expected.last,
                    expected.seen
                ),
                "case {case_index} grade {grade}"
            );
        }
    }
}

#[test]
fn bundled_catalog_should_preserve_all_entries_topics_and_media() {
    let catalog = Catalog::bundled().unwrap();
    assert_eq!(
        (
            catalog.entries().len(),
            catalog.topics().len(),
            catalog.entries().iter().filter(|e| e.m & 15 == 15).count()
        ),
        (7298, 19, 7298)
    );
}

#[test]
fn search_should_find_chinese_meanings_without_a_network() {
    let catalog = Catalog::bundled().unwrap();
    assert!(
        catalog
            .search("开花", None, None)
            .iter()
            .any(|e| e.id == "bloom")
    );
}

#[test]
fn journey_should_introduce_five_words_then_recall_those_five() {
    let catalog = Catalog::bundled().unwrap();
    let p = Progress::new(catalog.id());
    let journey = session::build(
        &catalog,
        &p,
        &Settings::default(),
        1_700_000_000_000,
        "2023-11-14",
        false,
        None,
    );
    assert!(
        journey.steps[..5].iter().all(|s| s.kind == StepKind::Intro)
            && journey.steps[5..10]
                .iter()
                .all(|s| s.kind != StepKind::Intro)
    );
}

#[test]
fn a_reopened_pending_answer_should_not_grade_the_word_twice() {
    let catalog = Catalog::bundled().unwrap();
    let settings = Settings::default();
    let mut progress = Progress::new(catalog.id());
    let now = 1_700_000_000_000;
    progress.session = Some(session::build(
        &catalog,
        &progress,
        &settings,
        now,
        "2023-11-14",
        false,
        None,
    ));
    for _ in 0..5 {
        session::advance(&mut progress, 2.0, now, "2023-11-14").unwrap();
    }
    let step = progress.session.as_ref().unwrap().steps[5].clone();
    let value = if step.kind == StepKind::Spell {
        catalog.entry(&step.id).unwrap().w.clone()
    } else {
        step.id.clone()
    };
    session::answer(
        &catalog,
        &mut progress,
        &settings,
        session::Submission {
            value: &value,
            milliseconds: 3000,
            hinted: false,
            now,
            day: "2023-11-14",
        },
    )
    .unwrap();
    let mut reopened: Progress =
        serde_json::from_str(&serde_json::to_string(&progress).unwrap()).unwrap();
    assert!(matches!(
        session::answer(
            &catalog,
            &mut reopened,
            &settings,
            session::Submission {
                value: &value,
                milliseconds: 3000,
                hinted: false,
                now,
                day: "2023-11-14"
            },
        ),
        Err(session::SessionError::AlreadyAnswered)
    ));
    assert_eq!(reopened.days["2023-11-14"].r, 1);
}

#[test]
fn retries_should_not_increase_first_attempt_review_counts() {
    let catalog = Catalog::bundled().unwrap();
    let settings = Settings {
        daily_new: 1,
        ..Default::default()
    };
    let mut progress = Progress::new(catalog.id());
    let now = 1_700_000_000_000;
    progress.session = Some(session::build(
        &catalog,
        &progress,
        &settings,
        now,
        "2023-11-14",
        false,
        None,
    ));
    session::advance(&mut progress, 1.0, now, "2023-11-14").unwrap();
    let step = progress.session.as_ref().unwrap().steps[1].clone();
    let wrong = if step.kind == StepKind::Spell {
        "wrong".into()
    } else {
        step.options
            .as_ref()
            .unwrap()
            .iter()
            .find(|id| *id != &step.id)
            .unwrap()
            .clone()
    };
    session::answer(
        &catalog,
        &mut progress,
        &settings,
        session::Submission {
            value: &wrong,
            milliseconds: 3000,
            hinted: false,
            now,
            day: "2023-11-14",
        },
    )
    .unwrap();
    session::advance(&mut progress, 1.0, now, "2023-11-14").unwrap();
    let retry = progress.session.as_ref().unwrap().steps[2].clone();
    session::answer(
        &catalog,
        &mut progress,
        &settings,
        session::Submission {
            value: &retry.id,
            milliseconds: 3000,
            hinted: false,
            now,
            day: "2023-11-14",
        },
    )
    .unwrap();
    assert_eq!(
        (
            progress.days["2023-11-14"].r,
            progress.days["2023-11-14"].c,
            progress.cards[&retry.id].reps
        ),
        (1, 0, 1)
    );
}

#[test]
fn typing_should_normalize_ligatures_and_mobile_typography() {
    assert!(typing::same("oﬃce — ‘go’", "office - 'go'"));
}

#[test]
fn typing_should_count_replacements_without_rewarding_deletion() {
    let mistake = typing::delta("ca", "cax", "cat");
    let deleted = typing::delta("cax", "ca", "cat");
    let corrected = typing::delta("ca", "cat", "cat");
    assert_eq!(
        typing::metrics(
            mistake.correct + deleted.correct + corrected.correct,
            mistake.total + deleted.total + corrected.total,
            3,
            60_000
        ),
        typing::Metrics {
            accuracy: 50,
            wpm: 1
        }
    );
}

#[test]
fn typing_should_use_unicode_codepoints_for_insertion_positions() {
    assert_eq!(
        typing::delta("😀", "😀a", "😀a"),
        typing::Delta {
            total: 1,
            correct: 1
        }
    );
}

#[test]
fn all_sentence_clozes_should_match_the_existing_web_rules() {
    use sha2::{Digest, Sha256};
    let catalog = Catalog::bundled().unwrap();
    let mut hash = Sha256::new();
    let mut count = 0;
    for word in catalog.entries() {
        for (index, example) in word.ex.iter().enumerate() {
            let found = atlas_domain::text::gap(&example.en, &word.w);
            let value = serde_json::json!([
                word.id,
                index,
                found.map(|g| g.before),
                found.map(|g| g.hidden),
                found.map(|g| g.after)
            ]);
            hash.update(value.to_string().as_bytes());
            hash.update(b"\n");
            count += 1;
        }
    }
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/cloze-web.json")).unwrap();
    assert_eq!(
        (count, format!("{:x}", hash.finalize())),
        (
            expected["examples"].as_u64().unwrap(),
            expected["sha256"].as_str().unwrap().to_owned()
        )
    );
}
