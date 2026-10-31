//! Failure and recovery checks for user learning history.
#![expect(
    clippy::unwrap_used,
    reason = "Malformed fixtures and failed assertions should fail tests loudly."
)]

use atlas_domain::{Catalog, Grade, scheduler};
use atlas_storage::{
    Backend, Error, FileBackend, SNAPSHOT_KEY, Snapshot, commit, confirm_import, export, load,
    preview_import, recover,
};
use std::{cell::RefCell, collections::BTreeMap};

#[derive(Default)]
struct Memory {
    entries: RefCell<BTreeMap<String, String>>,
    fail: bool,
}
impl Backend for Memory {
    fn get(&self, key: &str) -> Result<Option<String>, Error> {
        Ok(self.entries.borrow().get(key).cloned())
    }
    fn set(&self, key: &str, value: &str) -> Result<(), Error> {
        if self.fail {
            Err(Error::Write("simulated full disk".into()))
        } else {
            self.entries.borrow_mut().insert(key.into(), value.into());
            Ok(())
        }
    }
}

#[test]
fn failed_commit_should_retain_the_known_saved_state() {
    let catalog = Catalog::bundled().unwrap();
    let mut saved = Snapshot::new(&catalog);
    let original = saved.clone();
    let mut candidate = saved.clone();
    candidate.settings.daily_new = 20;
    assert!(
        commit(
            &Memory {
                fail: true,
                ..Default::default()
            },
            &catalog,
            &mut saved,
            candidate
        )
        .is_err()
    );
    assert_eq!(saved, original);
}

#[test]
fn web_backup_should_roundtrip_without_changing_memory_timestamps() {
    let catalog = Catalog::bundled().unwrap();
    let mut original = Snapshot::new(&catalog);
    original.settings.onboarded = true;
    original.progress.cards.insert(
        "bloom".into(),
        scheduler::review(None, Grade::Good, 1_700_000_000_000, 0.9).unwrap(),
    );
    assert_eq!(
        preview_import(
            &export(&original, "2026-10-30T00:00:00Z").unwrap(),
            &catalog
        )
        .unwrap(),
        original
    );
}

#[test]
fn a_corrupt_snapshot_should_not_be_replaced_by_an_empty_record() {
    let catalog = Catalog::bundled().unwrap();
    let backend = Memory::default();
    backend.set(SNAPSHOT_KEY, "broken json").unwrap();
    assert!(load(&backend, &catalog).is_err());
    assert_eq!(
        backend.get(SNAPSHOT_KEY).unwrap().as_deref(),
        Some("broken json")
    );
}

#[test]
fn confirmed_import_should_keep_a_recoverable_preimport_snapshot() {
    let catalog = Catalog::bundled().unwrap();
    let backend = Memory::default();
    let mut saved = Snapshot::new(&catalog);
    let original = saved.clone();
    let mut imported = saved.clone();
    imported.settings.daily_new = 30;
    confirm_import(&backend, &catalog, &mut saved, imported).unwrap();
    assert_eq!(recover(&backend, &catalog).unwrap(), original);
}

#[test]
fn an_unknown_dataset_should_be_rejected_before_any_write() {
    let catalog = Catalog::bundled().unwrap();
    let mut snapshot = Snapshot::new(&catalog);
    snapshot.progress.dataset_id = "other-dataset".into();
    assert!(preview_import(&export(&snapshot, "").unwrap(), &catalog).is_err());
}

#[test]
fn native_file_backend_should_survive_close_and_reopen() {
    let root = std::env::temp_dir().join(format!(
        "word-atlas-persistence-{}-{}",
        std::process::id(),
        chrono_id()
    ));
    let catalog = Catalog::bundled().unwrap();
    {
        let backend = FileBackend::open(&root).unwrap();
        let mut saved = Snapshot::new(&catalog);
        let mut candidate = saved.clone();
        candidate.settings.daily_new = 17;
        commit(&backend, &catalog, &mut saved, candidate).unwrap();
    }
    let reopened = FileBackend::open(&root).unwrap();
    assert_eq!(load(&reopened, &catalog).unwrap().settings.daily_new, 17);
    drop(reopened);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_second_process_backend_should_not_acquire_the_same_progress_lock() {
    let root = std::env::temp_dir().join(format!(
        "word-atlas-lock-{}-{}",
        std::process::id(),
        chrono_id()
    ));
    let first = FileBackend::open(&root).unwrap();
    assert!(FileBackend::open(&root).is_err());
    drop(first);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_confirmed_repair_should_preserve_the_original_corrupt_bytes() {
    let catalog = Catalog::bundled().unwrap();
    let backend = Memory::default();
    backend
        .set(SNAPSHOT_KEY, "{broken original learning file")
        .unwrap();
    let candidate = Snapshot::new(&catalog);
    atlas_storage::repair_import(&backend, &catalog, candidate.clone()).unwrap();
    assert_eq!(load(&backend, &catalog).unwrap(), candidate);
    assert_eq!(
        backend.get(atlas_storage::RECOVERY_KEY).unwrap().as_deref(),
        Some("{broken original learning file")
    );
}

fn chrono_id() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}
