//! Real Rust-root interaction tests; no TSX, browser or JavaScript VM.
#![expect(
    clippy::unwrap_used,
    reason = "Failed UI steps should fail these tests with their diagnostic."
)]

use atlas_domain::Catalog;
use atlas_storage::{Backend, DATABASE_FILE, FileBackend, SNAPSHOT_KEY, Snapshot, SqliteBackend};
use nsh_core::{
    element::{EngineContext, Root},
    prelude::*,
};
use nsh_engine::EngineConfig;
use nsh_testing::{DirectoryAssets, Headless};
use std::{path::PathBuf, sync::Arc};
use word_atlas_native::{App, StorageDirectory};

struct Profile(PathBuf);
impl Profile {
    fn new(onboarded: bool) -> Self {
        let path = std::env::temp_dir().join(format!(
            "word-atlas-ui-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let catalog = Catalog::bundled().unwrap();
        let mut snapshot = Snapshot::new(&catalog);
        snapshot.settings.onboarded = onboarded;
        snapshot.settings.daily_new = 1;
        snapshot.settings.autoplay = false;
        snapshot.settings.sfx = false;
        snapshot.settings.ambience = "off".into();
        let backend = FileBackend::open(&path).unwrap();
        backend
            .set(SNAPSHOT_KEY, &serde_json::to_string(&snapshot).unwrap())
            .unwrap();
        Self(path)
    }
    fn snapshot(&self) -> Snapshot {
        SqliteBackend::inspect(self.0.join(DATABASE_FILE), &Catalog::bundled().unwrap()).unwrap()
    }
    fn start(&self, width: f32) -> Headless {
        let mut config = EngineConfig::new(Root::of::<App>());
        config.context = EngineContext::default().with(StorageDirectory(self.0.clone()));
        config.viewport = size(px(width), px(900.0));
        config.ui_sets = vec![nsh_base::registry(), word_atlas_native::registry()];
        config.assets = Some(Arc::new(DirectoryAssets::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../public"),
            vec![],
        )));
        Headless::start(config).unwrap()
    }
}
impl Drop for Profile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn onboarding_should_commit_preferences_before_showing_the_real_daily_plan() {
    let profile = Profile::new(false);
    let mut app = profile.start(1100.0);
    app.wait_for("开启我的词光之旅  ↗").unwrap();
    app.press("开启我的词光之旅  ↗").unwrap();
    app.wait_for("开始今天的旅程  ↗").unwrap();
    assert!(app.screen().shows("复习 0   ·   新词 1"));
}

#[test]
fn native_catalog_search_and_bookmark_should_survive_app_restart() {
    let profile = Profile::new(true);
    {
        let mut app = profile.start(1100.0);
        app.wait_for("单词图鉴").unwrap();
        app.press("label:单词图鉴").unwrap();
        app.type_text("label:搜索单词或中文释义", "resilient")
            .unwrap();
        app.wait_for("label:打开词卡 resilient").unwrap();
        app.press("label:打开词卡 resilient").unwrap();
        app.wait_for("☆ 收藏").unwrap();
        app.press("☆ 收藏").unwrap();
        app.wait_for("★ 已收藏").unwrap();
    }
    let catalog = Catalog::bundled().unwrap();
    let backend = SqliteBackend::open(&profile.0, &catalog).unwrap();
    assert!(
        atlas_storage::load(&backend, &catalog)
            .unwrap()
            .progress
            .bookmarks
            .contains(&"resilient".into())
    );
}

#[test]
fn learning_should_restore_a_paused_journey_after_reopening() {
    let profile = Profile::new(true);
    {
        let mut app = profile.start(1100.0);
        app.wait_for("开始今天的旅程  ↗").unwrap();
        app.press("开始今天的旅程  ↗").unwrap();
        app.wait_for("初次见面").unwrap();
        app.press("认识了，继续  →").unwrap();
        app.wait_for("← 暂停学习").unwrap();
        app.press("← 暂停学习").unwrap();
        app.wait_for("继续今天的旅程  ↗").unwrap();
    }
    let mut reopened = profile.start(1100.0);
    reopened.wait_for("继续今天的旅程  ↗").unwrap();
    reopened.press("继续今天的旅程  ↗").unwrap();
    assert!(reopened.screen().shows("2 / 2 步"));
}

#[test]
fn narrow_layout_should_keep_navigation_and_search_working_after_resize() {
    let profile = Profile::new(true);
    let mut app = profile.start(375.0);
    app.wait_for("图鉴").unwrap();
    app.press("label:图鉴").unwrap();
    app.type_text("label:搜索单词或中文释义", "bloom").unwrap();
    app.wait_for("label:打开词卡 bloom").unwrap();
    app.set_viewport(size(px(1100.0), px(900.0))).unwrap();
    assert!(app.screen().matches("label:打开词卡 bloom").unwrap());
}

#[test]
fn returning_from_a_word_card_should_preserve_the_catalog_search() {
    let profile = Profile::new(true);
    let mut app = profile.start(1100.0);
    app.wait_for("label:单词图鉴").unwrap();
    app.press("label:单词图鉴").unwrap();
    app.type_text("label:搜索单词或中文释义", "resilient")
        .unwrap();
    app.wait_for("label:打开词卡 resilient").unwrap();
    app.press("label:打开词卡 resilient").unwrap();
    app.wait_for("← 返回").unwrap();
    app.press("← 返回").unwrap();
    assert_eq!(
        app.screen().input_text("label:搜索单词或中文释义").unwrap(),
        Some("resilient")
    );
}

#[test]
fn a_complete_native_journey_should_commit_one_first_attempt_and_show_its_summary() {
    let profile = Profile::new(true);
    let mut app = profile.start(1100.0);
    app.wait_for("开始今天的旅程  ↗").unwrap();
    app.press("开始今天的旅程  ↗").unwrap();
    app.press("认识了，继续  →").unwrap();
    let snapshot = profile.snapshot();
    let step = &snapshot.progress.session.as_ref().unwrap().steps[1];
    let options = step.options.as_ref().unwrap();
    let index = options.iter().position(|id| id == &step.id).unwrap();
    let catalog = Catalog::bundled().unwrap();
    let entry = catalog.entry(&step.id).unwrap();
    let choice = if step.kind == atlas_domain::model::StepKind::Meaning {
        entry.senses[0].zh.clone()
    } else {
        entry.w.clone()
    };
    app.press(&format!("label:{}.  {choice}", index + 1))
        .unwrap();
    app.wait_for("答对了，稳稳向前。").unwrap();
    app.press("继续  →").unwrap();
    app.wait_for("漂亮的一程。").unwrap();
    let snapshot = profile.snapshot();
    assert_eq!(
        (
            snapshot.progress.cards.len(),
            snapshot.progress.days.values().map(|d| d.r).sum::<u32>(),
            snapshot.progress.days.values().map(|d| d.c).sum::<u32>()
        ),
        (1, 1, 1)
    );
}

#[test]
fn typing_a_full_practice_round_should_keep_focus_and_save_actual_character_metrics() {
    let profile = Profile::new(true);
    let mut app = profile.start(1100.0);
    app.wait_for("label:打字练习").unwrap();
    app.press("label:打字练习 >> nth=0").unwrap();
    let catalog = Catalog::bundled().unwrap();
    let day = chrono::Local::now().format("%Y-%m-%d").to_string();
    let start = atlas_domain::typing::hash_seed(&day) as usize % catalog.entries().len();
    for i in 0..20 {
        let word = &catalog.entries()[(start + i * 17) % catalog.entries().len()].w;
        app.type_text("label:练习输入", word).unwrap();
        assert_eq!(
            app.screen().input_text("label:练习输入").unwrap(),
            Some(word.as_str())
        );
        app.press("确认，下一项  →").unwrap();
    }
    app.wait_for("练习完成。").unwrap();
    let snapshot = profile.snapshot();
    assert_eq!(
        (
            snapshot.progress.typing.len(),
            snapshot.progress.typing[0].items,
            snapshot.progress.typing[0].accuracy
        ),
        (1, 20, 100)
    );
}

#[test]
fn importing_a_backup_should_wait_for_confirmation_and_keep_recovery_data() {
    let profile = Profile::new(true);
    let mut app = profile.start(1100.0);
    app.wait_for("label:我的").unwrap();
    app.press("label:我的").unwrap();
    let catalog = Catalog::bundled().unwrap();
    let mut imported = Snapshot::new(&catalog);
    imported.settings.daily_new = 30;
    imported.settings.ambience = "off".into();
    app.set_clipboard(Some(atlas_storage::export(&imported, "").unwrap()));
    app.press("从剪贴板导入").unwrap();
    app.wait_for("确认替换学习记录").unwrap();
    let before = profile.snapshot();
    assert_eq!(before.settings.daily_new, 1);
    app.press("确认替换学习记录").unwrap();
    let after = profile.snapshot();
    assert_eq!(after.settings.daily_new, 30);
    drop(app);
    let backend = SqliteBackend::open(&profile.0, &catalog).unwrap();
    assert_eq!(atlas_storage::recover(&backend, &catalog).unwrap(), before);
}

#[test]
fn corrupt_legacy_profile_should_recover_through_the_native_confirmation_screen() {
    let profile = Profile::new(true);
    std::fs::write(profile.0.join("progress.json"), "{broken legacy profile").unwrap();
    let mut app = profile.start(1100.0);
    app.wait_for("学习档案尚未打开").unwrap();
    let catalog = Catalog::bundled().unwrap();
    let mut restored = Snapshot::new(&catalog);
    restored.settings.daily_new = 23;
    restored.settings.ambience = "off".into();
    restored.settings.sfx = false;
    app.set_clipboard(Some(atlas_storage::export(&restored, "").unwrap()));
    app.press("从剪贴板预览恢复备份").unwrap();
    app.wait_for("确认从备份恢复").unwrap();
    app.press("确认从备份恢复").unwrap();
    app.wait_for("开始今天的旅程  ↗").unwrap();
    assert_eq!(profile.snapshot().settings.daily_new, 23);
    assert_eq!(
        std::fs::read_to_string(profile.0.join("progress.json")).unwrap(),
        "{broken legacy profile"
    );
}
