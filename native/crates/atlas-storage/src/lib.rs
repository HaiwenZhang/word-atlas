//! Validated snapshots and web-compatible backups with failure-aware, transactional commits.

use atlas_domain::{Catalog, Progress, Settings};
use serde::{Deserialize, Serialize};

/// Maximum accepted backup size, matching the web UI's 30 MiB limit.
pub const MAX_BACKUP_BYTES: usize = 30 * 1024 * 1024;
/// Storage key for the complete native snapshot.
pub const SNAPSHOT_KEY: &str = "word-atlas:native:v1";
/// Storage key retained before a backup replaces the current learning history.
pub const RECOVERY_KEY: &str = "word-atlas:native:before-import";

/// A storage or validation error shown to the user without discarding their existing data.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// SQLite could not open, read or atomically commit the database.
    #[error("学习数据库操作失败：{0}")]
    Sqlite(#[from] rusqlite::Error),
    /// A newer saved state must be loaded before another edit can be committed.
    #[error("学习记录已被更新，请重新打开应用后重试。")]
    Conflict,
    /// A platform file operation failed.
    #[error("本地文件操作失败：{0}")]
    Io(#[from] std::io::Error),
    /// JSON decoding or encoding failed.
    #[error("备份格式无法读取：{0}")]
    Json(#[from] serde_json::Error),
    /// The candidate snapshot is not safe to use.
    #[error("{0}")]
    Invalid(String),
    /// The platform could not commit this write.
    #[error("保存失败：{0}")]
    Write(String),
}

mod file;
pub use file::FileBackend;
mod sqlite;
pub use sqlite::{DATABASE_FILE, ReviewEvent, SqliteBackend};

/// One atomically stored unit of preferences and learning data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    /// Application preferences.
    pub settings: Settings,
    /// Version 2 progress, shared with the web application.
    pub progress: Progress,
    #[serde(skip)]
    revision: i64,
}
impl PartialEq for Snapshot {
    fn eq(&self, other: &Self) -> bool {
        self.settings == other.settings && self.progress == other.progress
    }
}
impl Snapshot {
    /// Initializes empty local progress for the supplied catalog.
    pub fn new(catalog: &Catalog) -> Self {
        Self {
            settings: Settings::default(),
            progress: Progress::new(catalog.id()),
            revision: 0,
        }
    }

    /// Optimistic concurrency revision, local to the database and excluded from JSON backups.
    pub fn revision(&self) -> i64 {
        self.revision
    }

    /// Rejects unsupported versions, broken sessions, unknown words and invalid counters.
    pub fn validate(&self, catalog: &Catalog) -> Result<(), Error> {
        let p = &self.progress;
        if p.version != 2 || p.dataset_id != catalog.id() {
            return Err(Error::Invalid("备份的版本或词库不匹配。".into()));
        }
        if !self.settings.is_valid() {
            return Err(Error::Invalid("学习设置包含不支持的值。".into()));
        }
        if p.cards
            .iter()
            .any(|(id, card)| catalog.entry(id).is_none() || !card.is_valid())
        {
            return Err(Error::Invalid("备份中有无效词条或记忆数据。".into()));
        }
        if p.bookmarks.len() > 20_000 || p.mistakes.len() > 300 || p.typing.len() > 500 {
            return Err(Error::Invalid("备份记录数量超过限制。".into()));
        }
        if p.bookmarks
            .iter()
            .chain(&p.mistakes)
            .any(|id| catalog.entry(id).is_none())
        {
            return Err(Error::Invalid("备份引用了当前词库不存在的词条。".into()));
        }
        if p.days
            .iter()
            .any(|(day, d)| chrono_day_invalid(day) || d.c > d.r || d.n > d.r)
        {
            return Err(Error::Invalid("每日学习统计无效。".into()));
        }
        if p.typing.iter().any(|t| {
            t.at < 0
                || !["word", "sentence", "dictation"].contains(&t.mode.as_str())
                || t.accuracy > 100
                || t.wpm > 400
        }) {
            return Err(Error::Invalid("打字练习记录无效。".into()));
        }
        if let Some(s) = &p.session
            && (s.id.is_empty()
                || s.started < 0
                || s.steps.len() > 2_000
                || s.index > s.steps.len()
                || s.finished.is_some_and(|t| t < s.started)
                || (s.finished.is_some() && s.index != s.steps.len())
                || (s.pending.is_some() && s.index >= s.steps.len())
                || s.steps.iter().any(|step| {
                    catalog.entry(&step.id).is_none()
                        || step.options.as_ref().is_some_and(|options| {
                            options.len() > 4
                                || options.iter().any(|id| catalog.entry(id).is_none())
                                || !options.contains(&step.id)
                        })
                })
                || s.new_ids
                    .iter()
                    .chain(&s.review_ids)
                    .chain(s.graded.keys())
                    .any(|id| catalog.entry(id).is_none()))
        {
            return Err(Error::Invalid("未完成的学习会话无效。".into()));
        }
        Ok(())
    }
}

fn chrono_day_invalid(day: &str) -> bool {
    day.len() != 10 || chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d").is_err()
}

/// Platform-independent backend: a failed write must retain the previous value.
pub trait Backend {
    /// Reads a value, returning an error instead of treating an unreadable file as empty.
    fn get(&self, key: &str) -> Result<Option<String>, Error>;
    /// Atomically replaces one key, returning an error on quota, permissions or disk failure.
    fn set(&self, key: &str, value: &str) -> Result<(), Error>;

    /// Reads typed progress. Legacy backends decode their JSON; SQLite reads normalized tables.
    fn read_snapshot(&self) -> Result<Option<Snapshot>, Error> {
        self.get(SNAPSHOT_KEY)?
            .map(|raw| serde_json::from_str(&raw).map_err(Error::from))
            .transpose()
    }

    /// Commits a complete logical edit; SQLite overrides this with one transaction and row-level diffs.
    fn apply(
        &self,
        expected: Option<&Snapshot>,
        candidate: &Snapshot,
        mode: WriteMode,
    ) -> Result<i64, Error> {
        match mode {
            WriteMode::Import => {
                if let Some(previous) = expected {
                    self.set(RECOVERY_KEY, &serde_json::to_string(previous)?)?;
                }
            }
            WriteMode::Repair => {
                if let Some(raw) = self.get(SNAPSHOT_KEY)? {
                    self.set(RECOVERY_KEY, &raw)?;
                }
            }
            WriteMode::Commit | WriteMode::Restore => {}
        }
        self.set(SNAPSHOT_KEY, &serde_json::to_string(candidate)?)?;
        Ok(0)
    }

    /// Restores a validated recovery snapshot. SQLite also restores its archived review history.
    fn restore(&self, catalog: &Catalog) -> Result<Snapshot, Error> {
        let raw = self
            .get(RECOVERY_KEY)?
            .ok_or_else(|| Error::Invalid("没有可恢复的迁移前快照。".into()))?;
        let mut snapshot: Snapshot = serde_json::from_str(&raw)?;
        snapshot.validate(catalog)?;
        snapshot.revision = self.apply(None, &snapshot, WriteMode::Restore)?;
        Ok(snapshot)
    }
}

/// Intent of an atomic persistence operation, including its recovery/history policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteMode {
    /// A normal learning or settings edit.
    Commit,
    /// Explicit replacement from an external backup, retaining the previous state.
    Import,
    /// Restoring the previously retained state and its review history.
    Restore,
    /// Explicit repair after an unreadable legacy snapshot.
    Repair,
}

/// Parses a persisted snapshot without silently resetting malformed progress.
pub fn load(backend: &impl Backend, catalog: &Catalog) -> Result<Snapshot, Error> {
    match backend.read_snapshot()? {
        Some(snapshot) => {
            snapshot.validate(catalog)?;
            Ok(snapshot)
        }
        None => Ok(Snapshot::new(catalog)),
    }
}

/// Commits a validated candidate before replacing the caller's known-saved state.
pub fn commit(
    backend: &impl Backend,
    catalog: &Catalog,
    saved: &mut Snapshot,
    mut candidate: Snapshot,
) -> Result<(), Error> {
    candidate.validate(catalog)?;
    candidate.revision = backend.apply(Some(saved), &candidate, WriteMode::Commit)?;
    *saved = candidate;
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct Backup {
    app: String,
    version: u8,
    #[serde(default)]
    exported: String,
    progress: Progress,
    settings: Settings,
}

/// Exports a web-compatible backup; active sessions remain local to the originating device.
pub fn export(snapshot: &Snapshot, exported: &str) -> Result<String, Error> {
    let mut progress = snapshot.progress.clone();
    progress.session = None;
    Ok(serde_json::to_string_pretty(&Backup {
        app: "word-atlas".into(),
        version: 2,
        exported: exported.into(),
        progress,
        settings: snapshot.settings.clone(),
    })?)
}

/// Reads a candidate for a confirmation preview, without touching the stored progress.
pub fn preview_import(raw: &str, catalog: &Catalog) -> Result<Snapshot, Error> {
    if raw.len() > MAX_BACKUP_BYTES {
        return Err(Error::Invalid("文件大于 30 MB，不像是学习备份。".into()));
    }
    let backup: Backup = serde_json::from_str(raw)?;
    if backup.app != "word-atlas" || backup.version != 2 {
        return Err(Error::Invalid("这不是兼容的词光学习备份。".into()));
    }
    let mut snapshot = Snapshot {
        settings: backup.settings,
        progress: backup.progress,
        revision: 0,
    };
    snapshot.progress.session = None;
    snapshot.settings.onboarded = true;
    snapshot.validate(catalog)?;
    Ok(snapshot)
}

/// Retains the current state for recovery, then replaces it with the explicitly confirmed import.
pub fn confirm_import(
    backend: &impl Backend,
    catalog: &Catalog,
    saved: &mut Snapshot,
    mut candidate: Snapshot,
) -> Result<(), Error> {
    candidate.validate(catalog)?;
    candidate.revision = backend.apply(Some(saved), &candidate, WriteMode::Import)?;
    *saved = candidate;
    Ok(())
}

/// Restores the pre-import snapshot only after it passes current validation.
pub fn recover(backend: &impl Backend, catalog: &Catalog) -> Result<Snapshot, Error> {
    backend.restore(catalog)
}

/// Repairs an unreadable JSON snapshot after an explicitly confirmed, validated backup import.
/// The original bytes are preserved even when they cannot be decoded as a valid snapshot.
pub fn repair_import(
    backend: &impl Backend,
    catalog: &Catalog,
    mut candidate: Snapshot,
) -> Result<Snapshot, Error> {
    candidate.validate(catalog)?;
    candidate.revision = backend.apply(None, &candidate, WriteMode::Repair)?;
    Ok(candidate)
}
