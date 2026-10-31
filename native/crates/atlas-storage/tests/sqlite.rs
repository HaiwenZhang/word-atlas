//! Migration, atomic failure, concurrency and durable native progress behavior.
#![expect(
    clippy::unwrap_used,
    reason = "A failed database step must fail its test with diagnostics."
)]

use std::path::PathBuf;

use atlas_domain::{
    Catalog, Grade,
    model::{DayStat, TypingRound},
    scheduler, session,
};
use atlas_storage::{
    Backend, DATABASE_FILE, Error, FileBackend, RECOVERY_KEY, SNAPSHOT_KEY, Snapshot,
    SqliteBackend, commit, confirm_import, load, recover,
};
use rusqlite::Connection;

struct Profile(PathBuf);
impl Profile {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "word-atlas-sqlite-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn sqlite(&self, catalog: &Catalog) -> SqliteBackend {
        SqliteBackend::open(&self.0, catalog).unwrap()
    }
    fn connection(&self) -> Connection {
        Connection::open(self.0.join(DATABASE_FILE)).unwrap()
    }
    fn seed_json(&self, snapshot: &Snapshot) -> String {
        let raw = serde_json::to_string_pretty(snapshot).unwrap();
        FileBackend::open(&self.0)
            .unwrap()
            .set(SNAPSHOT_KEY, &raw)
            .unwrap();
        raw
    }
}
impl Drop for Profile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn answer_one(catalog: &Catalog, snapshot: &mut Snapshot) {
    let now = 1_700_000_000_000;
    let day = "2023-11-14";
    snapshot.settings.daily_new = 1;
    snapshot.progress.session = Some(session::build(
        catalog,
        &snapshot.progress,
        &snapshot.settings,
        now,
        day,
        false,
        None,
    ));
    session::advance(&mut snapshot.progress, 1.0, now, day).unwrap();
    let step = snapshot.progress.session.as_ref().unwrap().steps[1].clone();
    session::answer(
        catalog,
        &mut snapshot.progress,
        &snapshot.settings,
        session::Submission {
            value: &step.id,
            milliseconds: 3000,
            hinted: false,
            now,
            day,
        },
    )
    .unwrap();
}

#[test]
fn migration_should_preserve_every_legacy_field_and_keep_the_original_json() {
    let profile = Profile::new();
    let catalog = Catalog::bundled().unwrap();
    let mut original = Snapshot::new(&catalog);
    original.settings.daily_new = 17;
    original.settings.onboarded = true;
    original.progress.cards.insert(
        "bloom".into(),
        scheduler::review(None, Grade::Good, 1_700_000_000_000, 0.9).unwrap(),
    );
    original.progress.bookmarks = vec!["bloom".into(), "resilient".into()];
    original.progress.mistakes = vec!["resilient".into()];
    original.progress.days.insert(
        "2023-11-14".into(),
        DayStat {
            n: 1,
            r: 1,
            c: 1,
            t: 63,
        },
    );
    original.progress.typing.push(TypingRound {
        at: 1_700_000_000_000,
        mode: "sentence".into(),
        items: 20,
        wpm: 42,
        accuracy: 95,
    });
    original.progress.session = Some(session::build(
        &catalog,
        &original.progress,
        &original.settings,
        1_700_000_001_000,
        "2023-11-14",
        false,
        None,
    ));
    let source = profile.seed_json(&original);
    let backend = profile.sqlite(&catalog);
    assert_eq!(load(&backend, &catalog).unwrap(), original);
    assert_eq!(
        std::fs::read_to_string(profile.0.join("progress.json")).unwrap(),
        source
    );
    assert_eq!(
        backend.review_events().unwrap().len(),
        0,
        "migration must not invent historical reviews"
    );
    assert_eq!(
        &std::fs::read(backend.path()).unwrap()[..16],
        b"SQLite format 3\0"
    );
}

#[test]
fn legacy_json_should_not_override_an_already_migrated_database() {
    let profile = Profile::new();
    let catalog = Catalog::bundled().unwrap();
    let mut original = Snapshot::new(&catalog);
    original.settings.daily_new = 17;
    profile.seed_json(&original);
    drop(profile.sqlite(&catalog));
    original.settings.daily_new = 77;
    profile.seed_json(&original);
    let backend = profile.sqlite(&catalog);
    assert_eq!(load(&backend, &catalog).unwrap().settings.daily_new, 17);
}

#[test]
fn first_answer_should_save_card_statistics_pending_feedback_and_review_history_together() {
    let profile = Profile::new();
    let catalog = Catalog::bundled().unwrap();
    let backend = profile.sqlite(&catalog);
    let mut saved = load(&backend, &catalog).unwrap();
    let mut candidate = saved.clone();
    answer_one(&catalog, &mut candidate);
    commit(&backend, &catalog, &mut saved, candidate).unwrap();
    drop(backend);
    let reopened = profile.sqlite(&catalog);
    let actual = load(&reopened, &catalog).unwrap();
    assert_eq!(actual, saved);
    assert!(actual.progress.session.as_ref().unwrap().pending.is_some());
    assert_eq!(
        (
            actual.progress.cards.len(),
            actual.progress.days["2023-11-14"].r,
            reopened.review_events().unwrap().len()
        ),
        (1, 1, 1)
    );
}

#[test]
fn a_midtransaction_failure_should_roll_back_every_table_and_leave_memory_unchanged() {
    let profile = Profile::new();
    let catalog = Catalog::bundled().unwrap();
    let backend = profile.sqlite(&catalog);
    let mut saved = load(&backend, &catalog).unwrap();
    let original = saved.clone();
    let revision = saved.revision();
    let mut candidate = saved.clone();
    answer_one(&catalog, &mut candidate);
    let connection = profile.connection();
    connection.execute_batch("CREATE TRIGGER reject_day BEFORE INSERT ON day_stats BEGIN SELECT RAISE(ABORT,'simulated disk error'); END;").unwrap();
    assert!(commit(&backend, &catalog, &mut saved, candidate.clone()).is_err());
    assert_eq!(saved, original);
    assert_eq!(saved.revision(), revision);
    assert_eq!(
        SqliteBackend::inspect(backend.path(), &catalog).unwrap(),
        original
    );
    assert!(backend.review_events().unwrap().is_empty());
    connection.execute_batch("DROP TRIGGER reject_day").unwrap();
    commit(&backend, &catalog, &mut saved, candidate).unwrap();
    assert_eq!(
        backend.review_events().unwrap().len(),
        1,
        "retrying a failed transaction must not double-record a review"
    );
}

#[test]
fn unrelated_cards_should_not_be_rewritten_when_one_card_changes() {
    let profile = Profile::new();
    let catalog = Catalog::bundled().unwrap();
    let mut original = Snapshot::new(&catalog);
    let now = 1_700_000_000_000;
    for id in ["bloom", "resilient"] {
        original.progress.cards.insert(
            id.into(),
            scheduler::review(None, Grade::Easy, now, 0.9).unwrap(),
        );
    }
    profile.seed_json(&original);
    let backend = profile.sqlite(&catalog);
    let mut saved = load(&backend, &catalog).unwrap();
    let mut candidate = saved.clone();
    candidate.progress.cards.insert(
        "resilient".into(),
        scheduler::review(
            saved.progress.cards.get("resilient"),
            Grade::Good,
            now + 86_400_000,
            0.9,
        )
        .unwrap(),
    );
    profile.connection().execute_batch("CREATE TRIGGER protect_bloom BEFORE UPDATE ON cards WHEN NEW.word_id='bloom' BEGIN SELECT RAISE(ABORT,'unrelated card rewritten'); END;").unwrap();
    commit(&backend, &catalog, &mut saved, candidate).unwrap();
    assert_eq!(
        load(&backend, &catalog).unwrap().progress.cards["bloom"],
        original.progress.cards["bloom"]
    );
}

#[test]
fn stale_snapshot_should_not_overwrite_a_newer_commit() {
    let profile = Profile::new();
    let catalog = Catalog::bundled().unwrap();
    let backend = profile.sqlite(&catalog);
    let mut saved = load(&backend, &catalog).unwrap();
    let mut stale = saved.clone();
    let mut first = saved.clone();
    first.settings.daily_new = 17;
    commit(&backend, &catalog, &mut saved, first).unwrap();
    let mut second = stale.clone();
    second.settings.daily_new = 20;
    assert!(matches!(
        commit(&backend, &catalog, &mut stale, second),
        Err(Error::Conflict)
    ));
    assert_eq!(load(&backend, &catalog).unwrap().settings.daily_new, 17);
}

#[test]
fn failed_import_should_retain_the_previous_recovery_snapshot_and_its_review_history() {
    let profile = Profile::new();
    let catalog = Catalog::bundled().unwrap();
    let backend = profile.sqlite(&catalog);
    let mut saved = load(&backend, &catalog).unwrap();
    let mut graded = saved.clone();
    answer_one(&catalog, &mut graded);
    commit(&backend, &catalog, &mut saved, graded).unwrap();
    let original = saved.clone();
    let reviews = backend.review_events().unwrap();
    let mut imported = Snapshot::new(&catalog);
    imported.settings.daily_new = 30;
    confirm_import(&backend, &catalog, &mut saved, imported).unwrap();
    let after_first_import = saved.clone();
    profile.connection().execute_batch("CREATE TRIGGER reject_settings BEFORE UPDATE ON settings BEGIN SELECT RAISE(ABORT,'import failed'); END;").unwrap();
    let mut replacement = Snapshot::new(&catalog);
    replacement.settings.daily_new = 20;
    assert!(confirm_import(&backend, &catalog, &mut saved, replacement).is_err());
    assert_eq!(load(&backend, &catalog).unwrap(), after_first_import);
    profile
        .connection()
        .execute_batch("DROP TRIGGER reject_settings")
        .unwrap();
    assert_eq!(recover(&backend, &catalog).unwrap(), original);
    assert_eq!(backend.review_events().unwrap(), reviews);
}

#[test]
fn invalid_legacy_json_should_require_explicit_repair_and_remain_available() {
    let profile = Profile::new();
    let catalog = Catalog::bundled().unwrap();
    let broken = "{broken old profile";
    std::fs::write(profile.0.join("progress.json"), broken).unwrap();
    assert!(SqliteBackend::open(&profile.0, &catalog).is_err());
    let repair = SqliteBackend::open_for_repair(&profile.0).unwrap();
    atlas_storage::repair_import(&repair, &catalog, Snapshot::new(&catalog)).unwrap();
    assert_eq!(repair.get(RECOVERY_KEY).unwrap().as_deref(), Some(broken));
    drop(repair);
    let backend = profile.sqlite(&catalog);
    assert!(load(&backend, &catalog).is_ok());
    assert_eq!(
        std::fs::read_to_string(profile.0.join("progress.json")).unwrap(),
        broken
    );
}

#[test]
fn newer_database_schema_should_be_refused_without_reinitializing_it() {
    let profile = Profile::new();
    let catalog = Catalog::bundled().unwrap();
    drop(profile.sqlite(&catalog));
    {
        let connection = profile.connection();
        connection.execute_batch("PRAGMA user_version=99").unwrap();
    }
    let bytes = std::fs::read(profile.0.join(DATABASE_FILE)).unwrap();
    assert!(SqliteBackend::open(&profile.0, &catalog).is_err());
    assert_eq!(std::fs::read(profile.0.join(DATABASE_FILE)).unwrap(), bytes);
}

#[test]
fn corrupt_sqlite_should_not_fall_back_to_a_stale_legacy_profile() {
    let profile = Profile::new();
    let catalog = Catalog::bundled().unwrap();
    profile.seed_json(&Snapshot::new(&catalog));
    let bytes = b"not a SQLite database";
    std::fs::write(profile.0.join(DATABASE_FILE), bytes).unwrap();
    assert!(SqliteBackend::open(&profile.0, &catalog).is_err());
    assert_eq!(std::fs::read(profile.0.join(DATABASE_FILE)).unwrap(), bytes);
}

#[test]
fn full_catalog_progress_should_migrate_without_truncating_cards() {
    let profile = Profile::new();
    let catalog = Catalog::bundled().unwrap();
    let mut original = Snapshot::new(&catalog);
    let card = scheduler::review(None, Grade::Easy, 1_700_000_000_000, 0.9).unwrap();
    for word in catalog.entries() {
        original
            .progress
            .cards
            .insert(word.id.clone(), card.clone());
    }
    profile.seed_json(&original);
    let backend = profile.sqlite(&catalog);
    let migrated = load(&backend, &catalog).unwrap();
    assert_eq!(migrated, original);
    assert_eq!(migrated.progress.cards.len(), 7298);
}

#[test]
fn resetting_progress_should_also_clear_native_review_history() {
    let profile = Profile::new();
    let catalog = Catalog::bundled().unwrap();
    let backend = profile.sqlite(&catalog);
    let mut saved = load(&backend, &catalog).unwrap();
    let mut graded = saved.clone();
    answer_one(&catalog, &mut graded);
    commit(&backend, &catalog, &mut saved, graded).unwrap();
    assert_eq!(backend.review_events().unwrap().len(), 1);
    let mut reset = saved.clone();
    reset.progress = atlas_domain::Progress::new(catalog.id());
    commit(&backend, &catalog, &mut saved, reset).unwrap();
    assert!(backend.review_events().unwrap().is_empty());
}

#[test]
fn process_exit_before_commit_should_leave_the_saved_progress_intact() {
    let profile = Profile::new();
    let catalog = Catalog::bundled().unwrap();
    let backend = profile.sqlite(&catalog);
    let original = load(&backend, &catalog).unwrap();
    drop(backend);
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "crash_child", "--ignored"])
        .env("WORD_ATLAS_SQLITE_CRASH_TEST", &profile.0)
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(73));
    assert_eq!(load(&profile.sqlite(&catalog), &catalog).unwrap(), original);
}

#[test]
#[ignore = "invoked only by the process-exit durability test with its disposable profile"]
fn crash_child() {
    let path = std::env::var_os("WORD_ATLAS_SQLITE_CRASH_TEST").unwrap();
    let connection = Connection::open(PathBuf::from(path).join(DATABASE_FILE)).unwrap();
    connection
        .execute_batch("BEGIN IMMEDIATE; UPDATE settings SET value='77' WHERE key='dailyNew';")
        .unwrap();
    connection.cache_flush().unwrap();
    std::process::exit(73);
}
