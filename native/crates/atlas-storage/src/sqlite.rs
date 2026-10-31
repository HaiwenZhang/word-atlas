//! Normalized SQLite progress, incremental writes and atomic history/recovery operations.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use atlas_domain::{Catalog, Grade};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::{Backend, Error, FileBackend, RECOVERY_KEY, SNAPSHOT_KEY, Snapshot, WriteMode};

mod rows;

/// Database filename inside the application's private data directory.
pub const DATABASE_FILE: &str = "progress.sqlite3";
const APPLICATION_ID: i64 = 0x5741_544c;
const SCHEMA_VERSION: i64 = 1;

struct Database {
    connection: Connection,
    cache: Option<Snapshot>,
}

/// Shared native SQLite connection. Clones serialize access and retain the profile process lock.
#[derive(Clone)]
pub struct SqliteBackend {
    database: Arc<Mutex<Database>>,
    legacy: FileBackend,
    path: PathBuf,
}

/// A first-attempt review recorded by this native client, excluding imported or reconstructed history.
#[derive(Debug, Clone, PartialEq)]
pub struct ReviewEvent {
    /// Stable vocabulary identifier.
    pub word_id: String,
    /// Actual rating time, Unix milliseconds.
    pub reviewed_at: i64,
    /// First-attempt FSRS rating.
    pub grade: Grade,
    /// Stability before the rating; absent for a new card.
    pub previous_stability: Option<f64>,
    /// Resulting stability in days.
    pub stability: f64,
    /// Resulting difficulty.
    pub difficulty: f64,
    /// Resulting review due time.
    pub due: i64,
    /// Resulting lifetime review count.
    pub reps: u32,
}

impl SqliteBackend {
    /// Opens the database and migrates a valid legacy JSON profile exactly once, retaining its files.
    /// An unreadable or incompatible database/profile produces an error instead of an empty reset.
    pub fn open(directory: impl Into<PathBuf>, catalog: &Catalog) -> Result<Self, Error> {
        let backend = Self::open_for_repair(directory)?;
        backend.initialize_profile(catalog)?;
        let snapshot = crate::load(&backend, catalog)?;
        backend.lock()?.cache = Some(snapshot);
        Ok(backend)
    }

    /// Opens the schema without importing legacy JSON, for an explicitly confirmed recovery.
    /// Corrupt or newer SQLite databases are still refused and never overwritten.
    pub fn open_for_repair(directory: impl Into<PathBuf>) -> Result<Self, Error> {
        let legacy = FileBackend::open(directory)?;
        let path = legacy.directory().join(DATABASE_FILE);
        let mut connection = Connection::open(&path)?;
        connection.busy_timeout(Duration::from_secs(3))?;
        initialize_schema(&mut connection)?;
        connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON; PRAGMA trusted_schema=OFF; PRAGMA wal_autocheckpoint=200;")?;
        let integrity: String = connection.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
        if integrity != "ok" {
            return Err(Error::Invalid(format!(
                "学习数据库校验失败：{integrity}；原文件已保留。"
            )));
        }
        Ok(Self {
            database: Arc::new(Mutex::new(Database {
                connection,
                cache: None,
            })),
            legacy,
            path,
        })
    }

    /// The live database file. Its WAL sidecar is part of a live database, so use JSON export for sharing.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reads a consistent snapshot without taking the writer's profile lock or changing the database.
    pub fn inspect(path: impl AsRef<Path>, catalog: &Catalog) -> Result<Snapshot, Error> {
        let mut connection = Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        connection.busy_timeout(Duration::from_secs(3))?;
        let tx = connection.transaction()?;
        let application: i64 = tx.query_row("PRAGMA application_id", [], |r| r.get(0))?;
        let version: i64 = tx.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if application != APPLICATION_ID || version != SCHEMA_VERSION {
            return Err(Error::Invalid("不是受支持的词光学习数据库。".into()));
        }
        let snapshot = rows::read_snapshot(&tx)?
            .ok_or_else(|| Error::Invalid("数据库没有已启用的学习档案。".into()))?;
        snapshot.validate(catalog)?;
        tx.commit()?;
        Ok(snapshot)
    }

    /// Reads durable review events in their original insertion order.
    pub fn review_events(&self) -> Result<Vec<ReviewEvent>, Error> {
        let db = self.lock()?;
        let mut query=db.connection.prepare("SELECT word_id,reviewed_at,grade,previous_stability,stability,difficulty,due,reps FROM review_log ORDER BY id")?;
        let records = query.query_map([], |row| {
            let grade: u8 = row.get(2)?;
            Ok((
                row.get(0)?,
                row.get(1)?,
                grade,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
            ))
        })?;
        let mut events = Vec::new();
        for record in records {
            let (word_id, reviewed_at, grade, previous_stability, stability, difficulty, due, reps) =
                record?;
            let grade = match grade {
                1 => Grade::Again,
                2 => Grade::Hard,
                3 => Grade::Good,
                4 => Grade::Easy,
                _ => return Err(Error::Invalid("复习记录评分无效。".into())),
            };
            events.push(ReviewEvent {
                word_id,
                reviewed_at,
                grade,
                previous_stability,
                stability,
                difficulty,
                due,
                reps,
            });
        }
        Ok(events)
    }

    fn lock(&self) -> Result<MutexGuard<'_, Database>, Error> {
        self.database
            .lock()
            .map_err(|_| Error::Write("数据库连接已中断，请重新打开应用。".into()))
    }

    fn initialize_profile(&self, catalog: &Catalog) -> Result<(), Error> {
        let mut db = self.lock()?;
        let tx = db
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if rows::read_snapshot(&tx)?.is_none() {
            let mut candidate = match self.legacy.get(SNAPSHOT_KEY)? {
                Some(raw) => serde_json::from_str::<Snapshot>(&raw)?,
                None => Snapshot::new(catalog),
            };
            candidate.validate(catalog)?;
            if let Some(raw) = self.legacy.get(RECOVERY_KEY)? {
                save_recovery(&tx, &raw)?;
            }
            candidate.revision = 1;
            rows::write_snapshot(&tx, None, &candidate, false)?;
        }
        tx.commit()?;
        Ok(())
    }

    fn apply_to_database(
        &self,
        expected: Option<&Snapshot>,
        candidate: &Snapshot,
        mode: WriteMode,
    ) -> Result<i64, Error> {
        let mut db = self.lock()?;
        let cache = db.cache.take();
        let original = if mode == WriteMode::Repair {
            self.legacy.get(SNAPSHOT_KEY)?
        } else {
            None
        };
        let result = (|| -> Result<i64, Error> {
            let tx = db
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)?;
            let revision = rows::revision(&tx)?;
            if let Some(expected) = expected
                && expected.revision != revision
            {
                return Err(Error::Conflict);
            }
            let current = match cache.as_ref().filter(|cached| cached.revision == revision) {
                Some(cached) => Some(cached.clone()),
                None => rows::read_snapshot(&tx)?,
            };
            match mode {
                WriteMode::Import|WriteMode::Repair=>{
                    let raw=match &current {Some(snapshot)=>Some(serde_json::to_string(snapshot)?),None=>original};
                    if let Some(raw)=raw {save_recovery(&tx,&raw)?;}
                    tx.execute_batch("DELETE FROM recovery_reviews; INSERT INTO recovery_reviews SELECT * FROM review_log; DELETE FROM review_log;")?;
                },
                WriteMode::Restore=>tx.execute_batch("DELETE FROM review_log; INSERT INTO review_log SELECT * FROM recovery_reviews;")?,
                WriteMode::Commit=>{
                    let empty=atlas_domain::Progress::new(&candidate.progress.dataset_id);
                    if candidate.progress==empty&&current.as_ref().is_some_and(|old|old.progress!=empty) {tx.execute("DELETE FROM review_log",[])?;}
                },
            }
            let next_revision = revision
                .checked_add(1)
                .ok_or_else(|| Error::Invalid("数据库版本计数已超出范围。".into()))?;
            let mut next = candidate.clone();
            next.revision = next_revision;
            rows::write_snapshot(&tx, current.as_ref(), &next, mode == WriteMode::Commit)?;
            tx.commit()?;
            Ok(next_revision)
        })();
        match result {
            Ok(revision) => {
                let mut stored = candidate.clone();
                stored.revision = revision;
                db.cache = Some(stored);
                Ok(revision)
            }
            Err(error) => {
                db.cache = cache;
                Err(error)
            }
        }
    }
}

impl Backend for SqliteBackend {
    fn get(&self, key: &str) -> Result<Option<String>, Error> {
        match key {
            SNAPSHOT_KEY => match self.read_snapshot()? {
                Some(snapshot) => Ok(Some(serde_json::to_string(&snapshot)?)),
                None => self.legacy.get(SNAPSHOT_KEY),
            },
            RECOVERY_KEY => Ok(self
                .lock()?
                .connection
                .query_row(
                    "SELECT raw_json FROM recovery_snapshot WHERE id=1",
                    [],
                    |row| row.get(0),
                )
                .optional()?),
            _ => Err(Error::Invalid("未知的学习档案键。".into())),
        }
    }

    fn set(&self, key: &str, value: &str) -> Result<(), Error> {
        // Raw key writes remain only for the legacy Backend contract. Application commits are typed.
        match key {
            SNAPSHOT_KEY => {
                let candidate: Snapshot = serde_json::from_str(value)?;
                let catalog = Catalog::bundled().map_err(|e| Error::Invalid(e.to_string()))?;
                candidate.validate(&catalog)?;
                let expected = self.read_snapshot()?;
                self.apply_to_database(expected.as_ref(), &candidate, WriteMode::Commit)?;
                Ok(())
            }
            RECOVERY_KEY => {
                save_recovery(&self.lock()?.connection, value)?;
                Ok(())
            }
            _ => Err(Error::Invalid("未知的学习档案键。".into())),
        }
    }

    fn read_snapshot(&self) -> Result<Option<Snapshot>, Error> {
        let mut db = self.lock()?;
        let tx = db.connection.transaction()?;
        let snapshot = rows::read_snapshot(&tx)?;
        tx.commit()?;
        db.cache = snapshot.clone();
        Ok(snapshot)
    }

    fn apply(
        &self,
        expected: Option<&Snapshot>,
        candidate: &Snapshot,
        mode: WriteMode,
    ) -> Result<i64, Error> {
        self.apply_to_database(expected, candidate, mode)
    }

    fn restore(&self, catalog: &Catalog) -> Result<Snapshot, Error> {
        // Validation and replacement share the same write transaction.
        let mut db = self.lock()?;
        let result = (|| -> Result<Snapshot, Error> {
            let tx = db
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)?;
            let raw: String = tx
                .query_row(
                    "SELECT raw_json FROM recovery_snapshot WHERE id=1",
                    [],
                    |row| row.get(0),
                )
                .optional()?
                .ok_or_else(|| Error::Invalid("没有可恢复的迁移前快照。".into()))?;
            let mut candidate: Snapshot = serde_json::from_str(&raw)?;
            candidate.validate(catalog)?;
            let current = rows::read_snapshot(&tx)?;
            candidate.revision = rows::revision(&tx)?
                .checked_add(1)
                .ok_or_else(|| Error::Invalid("数据库版本计数已超出范围。".into()))?;
            tx.execute_batch(
                "DELETE FROM review_log; INSERT INTO review_log SELECT * FROM recovery_reviews;",
            )?;
            rows::write_snapshot(&tx, current.as_ref(), &candidate, false)?;
            tx.commit()?;
            Ok(candidate)
        })();
        if let Ok(snapshot) = &result {
            db.cache = Some(snapshot.clone());
        }
        result
    }
}

fn save_recovery(connection: &Connection, raw: &str) -> Result<(), Error> {
    connection.execute("INSERT INTO recovery_snapshot(id,raw_json) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET raw_json=excluded.raw_json",params![raw])?;
    Ok(())
}

fn initialize_schema(connection: &mut Connection) -> Result<(), Error> {
    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let application: i64 = connection.query_row("PRAGMA application_id", [], |row| row.get(0))?;
    if !(0..=SCHEMA_VERSION).contains(&version) {
        return Err(Error::Invalid(format!(
            "数据库版本 {version} 不受当前客户端支持，原文件已保留。"
        )));
    }
    if application != 0 && application != APPLICATION_ID {
        return Err(Error::Invalid(
            "这个数据库不属于词光，已保留原文件。".into(),
        ));
    }
    if version == 0 {
        let tables: u32 = connection.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            [],
            |row| row.get(0),
        )?;
        if tables != 0 {
            return Err(Error::Invalid(
                "数据库含有未知数据表，不会自动覆盖。".into(),
            ));
        }
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch(include_str!("schema.sql"))?;
        tx.commit()?;
    } else if application != APPLICATION_ID {
        return Err(Error::Invalid("数据库缺少词光标识，原文件已保留。".into()));
    }
    Ok(())
}
