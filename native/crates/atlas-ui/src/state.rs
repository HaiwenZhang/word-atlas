//! Application actions: domain transitions commit before the visible saved state changes.

use std::{path::PathBuf, sync::Arc};

use atlas_domain::{Catalog, session};
use atlas_media::{AudioKind, audio};
use atlas_storage::{Snapshot, SqliteBackend};
use chrono::Local;
use nsh_core::prelude::*;

/// Override for embedded hosts and isolated native UI tests.
#[derive(Debug, Clone)]
pub struct StorageDirectory(pub PathBuf);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Page {
    Today,
    Atlas,
    Learn,
    Practice,
    Galaxy,
    Me,
    About,
}

#[derive(Clone)]
pub(crate) struct State {
    pub catalog: Arc<Catalog>,
    pub snapshot: Snapshot,
    pub backend: SqliteBackend,
    pub page: Page,
    pub detail: Option<String>,
    pub error: Option<String>,
    pub message: Option<String>,
    pub step_started: i64,
    pub audio: Option<(String, f32)>,
    pub audio_sequence: u64,
    pub cue: Option<(String, f32)>,
    pub cue_sequence: u64,
    pub pending_import: Option<Snapshot>,
}

pub(crate) fn now() -> i64 {
    Local::now().timestamp_millis()
}
pub(crate) fn day() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

pub(crate) fn directory() -> Result<PathBuf, String> {
    if let Some(path) = engine_context::<StorageDirectory>() {
        return Ok(path.0.clone());
    }
    if let Some(path) = std::env::var_os("WORD_ATLAS_DATA_DIR") {
        return Ok(PathBuf::from(path));
    }
    let app_id = "dev.wordatlas.client";
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    if let Some(home) = std::env::var_os("HOME") {
        return Ok(PathBuf::from(home)
            .join("Library/Application Support")
            .join(app_id));
    }
    #[cfg(target_os = "windows")]
    if let Some(root) = std::env::var_os("APPDATA") {
        return Ok(PathBuf::from(root).join(app_id));
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(root) = std::env::var_os("XDG_DATA_HOME") {
            return Ok(PathBuf::from(root).join(app_id));
        }
        if let Some(home) = std::env::var_os("HOME") {
            return Ok(PathBuf::from(home).join(".local/share").join(app_id));
        }
    }
    #[cfg(target_os = "android")]
    {
        use std::os::unix::fs::MetadataExt;
        // The NSH 0.3 Rust root has no ambient data-directory accessor. Respect Android's
        // per-user UID rather than assuming user 0. A custom host can supply StorageDirectory.
        let uid = std::fs::metadata("/proc/self")
            .map_err(|e| e.to_string())?
            .uid();
        let path = PathBuf::from(format!("/data/user/{}/{app_id}/files", uid / 100_000));
        if path.is_dir() {
            return Ok(path.join("word-atlas"));
        }
    }
    Err("找不到应用的私有数据目录，学习档案尚未打开。".into())
}

impl State {
    pub(crate) fn load() -> Result<Self, String> {
        let catalog = Arc::new(Catalog::bundled().map_err(|e| e.to_string())?);
        let backend = SqliteBackend::open(directory()?, &catalog).map_err(|e| e.to_string())?;
        let snapshot = atlas_storage::load(&backend, &catalog).map_err(|e| e.to_string())?;
        Ok(Self {
            catalog,
            snapshot,
            backend,
            page: Page::Today,
            detail: None,
            error: None,
            message: None,
            step_started: now(),
            audio: None,
            audio_sequence: 0,
            cue: None,
            cue_sequence: 0,
            pending_import: None,
        })
    }

    pub(crate) fn update(
        &mut self,
        action: impl FnOnce(&Catalog, &mut Snapshot) -> Result<(), String>,
    ) -> bool {
        let mut candidate = self.snapshot.clone();
        let result = action(&self.catalog, &mut candidate).and_then(|()| {
            atlas_storage::commit(&self.backend, &self.catalog, &mut self.snapshot, candidate)
                .map_err(|e| e.to_string())
        });
        match result {
            Ok(()) => {
                self.error = None;
                true
            }
            Err(error) => {
                self.error = Some(error);
                false
            }
        }
    }

    pub(crate) fn navigate(&mut self, page: Page) {
        self.page = page;
        self.detail = None;
        self.audio = None;
        self.error = None;
        self.sound("page", 0.4);
    }
    pub(crate) fn open_word(&mut self, id: &str) {
        self.detail = Some(id.into());
        self.audio = None;
    }

    pub(crate) fn start(&mut self, review_only: bool, ids: Option<Vec<String>>) {
        let time = now();
        let today = day();
        if self.update(|catalog, snapshot| {
            if ids.is_some()
                || snapshot
                    .progress
                    .session
                    .as_ref()
                    .is_none_or(|s| s.finished.is_some())
            {
                let journey = session::build(
                    catalog,
                    &snapshot.progress,
                    &snapshot.settings,
                    time,
                    &today,
                    review_only,
                    ids.as_deref(),
                );
                if journey.steps.is_empty() {
                    return Err("今天没有需要学习的词，可以浏览图鉴或练习。".into());
                }
                snapshot.progress.session = Some(journey);
            }
            Ok(())
        }) {
            self.page = Page::Learn;
            self.detail = None;
            self.step_started = time;
            self.autoplay();
            self.sound("start", 0.5);
        }
    }

    pub(crate) fn answer(&mut self, value: &str, hinted: bool) {
        let time = now();
        let elapsed = time.saturating_sub(self.step_started) as u64;
        let today = day();
        let mut correct = false;
        if self.update(|catalog, snapshot| {
            session::answer(
                catalog,
                &mut snapshot.progress,
                &snapshot.settings,
                session::Submission {
                    value,
                    milliseconds: elapsed,
                    hinted,
                    now: time,
                    day: &today,
                },
            )
            .map(|result| correct = result)
            .map_err(|e| e.to_string())
        }) {
            self.sound(if correct { "correct" } else { "wrong" }, 0.5);
            if let Some(id) = self.current_id() {
                self.play_word(&id, 1.0);
            }
        }
    }

    pub(crate) fn advance(&mut self) {
        let time = now();
        let seconds = time.saturating_sub(self.step_started) as f64 / 1000.0;
        let today = day();
        if self.update(|_, snapshot| {
            session::advance(&mut snapshot.progress, seconds, time, &today)
                .map_err(|e| e.to_string())
        }) {
            self.step_started = time;
            self.audio = None;
            self.autoplay();
            self.sound(
                if self
                    .snapshot
                    .progress
                    .session
                    .as_ref()
                    .is_some_and(|s| s.finished.is_some())
                {
                    "complete"
                } else {
                    "flip"
                },
                0.45,
            );
        }
    }

    pub(crate) fn current_id(&self) -> Option<String> {
        self.snapshot
            .progress
            .session
            .as_ref()
            .and_then(|s| s.steps.get(s.index))
            .map(|s| s.id.clone())
    }
    fn autoplay(&mut self) {
        if self.snapshot.settings.autoplay {
            let id = self.current_id();
            let pending = self
                .snapshot
                .progress
                .session
                .as_ref()
                .is_some_and(|s| s.pending.is_some());
            let should_play = self
                .snapshot
                .progress
                .session
                .as_ref()
                .and_then(|s| s.steps.get(s.index))
                .is_some_and(|step| {
                    matches!(
                        step.kind,
                        atlas_domain::model::StepKind::Intro
                            | atlas_domain::model::StepKind::Listen
                    )
                });
            if !pending
                && should_play
                && let Some(id) = id
            {
                self.play_word(&id, 1.0);
            }
        }
    }
    pub(crate) fn play_word(&mut self, id: &str, rate: f32) {
        let kind = if self.snapshot.settings.accent == "us" {
            AudioKind::Us
        } else {
            AudioKind::Uk
        };
        self.play(id, kind, rate);
    }
    pub(crate) fn play(&mut self, id: &str, kind: AudioKind, rate: f32) {
        match self.catalog.entry(id).map(|entry| audio(id, kind, entry.m)) {
            Some(Ok(Some(uri))) => {
                self.audio = Some((uri, rate));
                self.audio_sequence += 1;
            }
            _ => self.message = Some("这个词条没有对应录音。".into()),
        }
    }

    pub(crate) fn sound(&mut self, name: &str, volume: f32) {
        if self.snapshot.settings.sfx
            && let Ok(uri) = atlas_media::sound(name)
        {
            self.cue = Some((uri, volume));
            self.cue_sequence += 1;
        }
    }
}

pub(crate) type AppState = Signal<State>;
