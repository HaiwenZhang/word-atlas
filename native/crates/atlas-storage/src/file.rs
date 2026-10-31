//! Crash-resistant native persistence with an exclusive process lock and atomic replacement.

use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use crate::{Backend, Error, MAX_BACKUP_BYTES, RECOVERY_KEY, SNAPSHOT_KEY};

/// Local file storage. The shared lock prevents two app processes from overwriting each other's state.
#[derive(Debug, Clone)]
pub struct FileBackend {
    root: PathBuf,
    _lock: Arc<File>,
}

impl FileBackend {
    /// Opens an application-private directory and exclusively locks its learning state.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, Error> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join("learning.lock"))?;
        lock.try_lock().map_err(|e| {
            Error::Write(format!("学习档案已在另一个窗口进程打开，或无法锁定：{e}"))
        })?;
        Ok(Self {
            root,
            _lock: Arc::new(lock),
        })
    }

    /// The private directory containing snapshots and recovery data.
    pub fn directory(&self) -> &Path {
        &self.root
    }

    fn path(&self, key: &str) -> Result<PathBuf, Error> {
        match key {
            SNAPSHOT_KEY => Ok(self.root.join("progress.json")),
            RECOVERY_KEY => Ok(self.root.join("before-import.json")),
            _ => Err(Error::Invalid("未知的本地档案键。".into())),
        }
    }
}

impl Backend for FileBackend {
    fn get(&self, key: &str) -> Result<Option<String>, Error> {
        let path = self.path(key)?;
        let file = match File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        if file.metadata()?.len() > MAX_BACKUP_BYTES as u64 {
            return Err(Error::Invalid(
                "学习档案超过读取上限，已保留原文件。".into(),
            ));
        }
        let mut text = String::new();
        file.take(MAX_BACKUP_BYTES as u64 + 1)
            .read_to_string(&mut text)?;
        if text.len() > MAX_BACKUP_BYTES {
            return Err(Error::Invalid("学习档案在读取时超过大小上限。".into()));
        }
        Ok(Some(text))
    }

    fn set(&self, key: &str, value: &str) -> Result<(), Error> {
        if value.len() > MAX_BACKUP_BYTES {
            return Err(Error::Invalid("学习档案超过 30 MB，原文件已保留。".into()));
        }
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let target = self.path(key)?;
        let temporary = self.root.join(format!(
            ".progress-{}-{}.tmp",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let result = (|| -> Result<(), Error> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(value.as_bytes())?;
            file.sync_all()?;
            fs::rename(&temporary, target)?;
            // The data is already committed after rename. A directory fsync failure cannot roll it back.
            #[cfg(unix)]
            if let Ok(directory) = File::open(&self.root) {
                let _ = directory.sync_all();
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(temporary);
        }
        result
    }
}
