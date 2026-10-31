# SQLite 学习进度存储

客户端使用 `atlas-storage::SqliteBackend`；Rust 驱动固定为 `rusqlite = 0.41.1`，启用 `bundled`，SQLite 随应用编译。应用代码仍为 Rust，SQLite 引擎是 C 实现。图片、录音和词库继续作为内置资源，不放入用户进度数据库。

## 文件和表

应用私有数据目录中的 `progress.sqlite3` 是当前进度的唯一写入来源。开发和测试可通过 `WORD_ATLAS_DATA_DIR` 或 `StorageDirectory` 指定独立目录。沿用 `learning.lock` 防止旧版 JSON 客户端与新版同时写同一学习档案；同一进程内的连接访问串行化。

| 表 | 内容 |
| --- | --- |
| `profile` | 词库 ID、进度格式版本、乐观并发修订号 |
| `cards` | 每个词的全部 FSRS 字段和毫秒时间戳；按到期时间建索引 |
| `day_stats` | 每天的新词、首次作答、首次正确和活跃时间 |
| `ordered_words` | 保留顺序的收藏与最近错词 |
| `typing_rounds` | 最近 500 次练习的模式、时间、字符准确率和 WPM |
| `settings` | 独立设置项，以 JSON 标量保留原有值类型 |
| `active_session` | 进行中的步骤、首次评分、当前索引、待确认答案；复杂会话结构使用 JSON |
| `review_log` | 原生客户端实际发生的首次作答事件；按词条和时间建索引 |
| `recovery_snapshot` / `recovery_reviews` | 用户确认导入前的进度快照与复习历史 |

卡片、统计和列表为独立数据行。普通评分只写入发生变化的卡片、当天统计、当前会话和一条复习事件，不重写整本词库的卡片状态。重试练习不会重复写复习事件。重置进度会同时清除当前复习历史。

## 迁移旧进度

1. 打开 SQLite，检查词光数据库标识、`PRAGMA user_version` 和 `quick_check`。当前数据库 schema 版本为 1，学习进度 JSON 格式仍为 2。
2. 只有数据库尚无启用的学习档案时，才读取旧 `progress.json`，并按现有词库校验所有字段。
3. 在一个事务内写入卡片、会话、统计、收藏、练习与设置；旧 `before-import.json` 如存在，也保留到恢复区。
4. 迁移成功后 SQLite 成为权威来源，后续启动不会用遗留 JSON 覆盖数据库。原 JSON 不删除、不改写。

无效 JSON 不会被静默转换成空进度。用户可以从已有的有效备份预览并确认恢复；损坏的原始 JSON 会留在原路径，并在确认恢复时保存原始内容到数据库恢复区。损坏、来源未知或版本高于当前客户端的 SQLite 数据库直接报错并保留原文件，不自动重建，也不退回陈旧 JSON。

## 事务与失败处理

- 使用 WAL、`synchronous=FULL`、3 秒 busy timeout，并启用数据库约束。
- 一次答题的卡片、每日统计、待确认答案与复习事件在同一个 `BEGIN IMMEDIATE` 事务中提交。
- 保存成功后才替换界面持有的已保存状态；任意语句失败都会回滚，界面继续持有原状态并显示错误。
- 每次写入校验本地修订号，拒绝旧快照覆盖新提交。
- 导入前快照、历史归档和导入后的进度属于同一事务。失败的第二次导入不会破坏前一次恢复点。
- 恢复迁移前快照会同时恢复当时的原生复习事件。

## 备份兼容

继续通过原有 v2 JSON 格式与网页版交换卡片、统计、设置、收藏、错词和练习记录。导出不包含进行中的学习会话，和网页版行为一致。原生新增的逐次 `review_log` 保存在数据库及导入恢复区，不伪造网页迁移前不存在的历史事件。

运行中的 SQLite 可能还有 `-wal` / `-shm` 文件；不要只复制主文件当作实时备份。当前界面的备份入口导出经过校验的 JSON。数据库故障排查请先保留整个私有数据目录。

## 验证入口

```sh
cargo test --manifest-path native/Cargo.toml -p atlas-storage --locked
cargo test --manifest-path native/Cargo.toml -p word-atlas-native --test journeys --locked
cargo clippy --manifest-path native/Cargo.toml --workspace --all-targets --all-features --locked -- -D warnings
```

存储测试覆盖全量 7,298 张卡片迁移、旧 JSON 保留且不重复覆盖、事务中途故障、恢复点回滚、重启后的待确认答案、原生复习事件、旧修订号拒写、未知/损坏数据库拒绝，以及子进程在未提交事务中直接退出后的恢复。原生根组件测试直接读取 SQLite，验证收藏、学习、打字练习与导入确认流程。

Android SDK/NDK 暂不安装，安卓安装包仍待环境就绪后验证。此存储切换不代表整个跨平台客户端已完成验收。

## 本轮验证结果

- 存储：13 项 SQLite 测试和 8 项遗留后端/备份测试通过。
- 原生界面：9 项直接使用 SQLite 的 Rust 根组件测试通过。
- 领域与媒体：13 项测试通过，包含 FSRS 与全部例句填空的网页对照。
- 完整 Rust 测试合计 43 项有效用例通过；workspace Clippy 与格式检查通过。子进程崩溃辅助用例由其父测试主动调用，不独立执行。
- macOS 和 iOS 模拟器安装包均完成重新构建与全量资源哈希校验。
- iOS 26.5 模拟器已实际启动、迁移、重启；迁移卡片和收藏保持，旧 JSON 未被首次迁移改写；随后人为改成陈旧数据的 JSON 没有覆盖 SQLite。

本地输出：`output/native-sqlite-macos/词光.app`、`output/native-sqlite-ios/词光.app`。验证日志和资源报告在 `output/native-client/sqlite-*` 与 `ios-sqlite-runtime.json`。
