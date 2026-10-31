# 词光原生客户端 · 纯 Rust 方案

调研日期：2026-10-30。分支：`feat/nsh-client-design`。

## 结论与边界

**可行，推荐先做纯 Rust 技术验证，再迁移主学习流程。** 使用 NSH 的 Rust 模板和 Rust 组件 API，界面、状态、事件、学习逻辑均用 Rust。客户端不使用 TSX、React、QuickJS 或 WebView 包装现有网站。系统后端、打包入口仍会包含框架提供的原生平台代码；“纯 Rust”指我们维护的应用界面与业务代码。

原生工程已经建立，并生成过 macOS 和 iOS 模拟器安装包；macOS 窗口已实际打开。当前进度存储切换为 SQLite，详见 [SQLite 存储说明](sqlite-storage.md)。完整跨平台客户端仍在开发和验收中，不能把设计稿、一次构建或局部测试视为全部完成。

NSH 官方的 [Rust 指南](https://nsh.dev/docs/rust/) 明确支持 UI、状态和处理函数全部使用 Rust，应用运行时不携带 JavaScript VM；[Rust gallery](https://github.com/longbridge/nsh/tree/main/examples/gallery-rs) 演示同一套 Rust 组件在桌面侧栏与手机导航之间切换。

## 平台覆盖

| 平台 | 官方最低要求 | 后端与交付形式 |
| --- | --- | --- |
| macOS | macOS 14+ | AppKit；`.app` |
| Windows | Windows 10 1809+ | Win32 / Direct2D；应用目录、`.zip` |
| Linux | GTK 4.14+ | GTK 4；应用目录、`.tar.gz` |
| iOS | iOS 15+ | UIKit；模拟器 `.app`、签名 `.ipa` |
| Android | Android 9 / API 28+ | Android Views；签名 `.apk` |

来源：[平台列表](https://nsh.dev/docs/platforms/)、[打包指南](https://nsh.dev/docs/packaging/)。这不等于已完成各商店发布链路；Android 的 AAB、Windows 安装器、macOS 分发签名/公证和各商店要求需要单独验证。iOS 构建需要 Mac 与 Xcode，Android 需要 SDK，纯 Rust 构建还需要相应 Rust target 与 Android NDK。参见[安装指南](https://nsh.dev/docs/installation/)。

## 设计稿

`client-concepts.html` 是可比较的交互设计源稿，包含三种布局和桌面 / iOS / Android 预览；`preview.html` 是可直接用浏览器打开的独立预览。它们只是表现设计的工具，不是客户端运行代码。素材复用现有词光插画，学习数据为演示数据。

| 方向 | 桌面 | 手机 | 适用定位 |
| --- | --- | --- | --- |
| 暖纸图鉴（推荐） | 侧栏、今日计划、插画词卡、学习记录 | 今日计划优先，底部导航，大触控按钮 | 延续纸张、墨色、赤陶色品牌，适合首版 |
| 深空旅程 | 星图为主画布，话题与单词就地展开 | 简化星图与底部单词面板 | 品牌辨识度强，但绘图与性能验证成本更高 |
| 专注工作台 | 待复习列表、词卡、例句，键盘优先 | 逐词学习，隐藏桌面信息列 | 高频复习和桌面学习效率优先 |

交互：切换设计与设备；点击开始/词卡进入学习；显示释义；选择记忆程度后切换样例词；返回概览。示例不播放音频、不保存实际学习记录，避免把设计演示误认为已实现能力。专注工作台的桌面快捷键：空格显示释义，1–4 自评。

共同约束：不增加账号与追踪；离线可学习；云同步不默认承诺；手机考虑安全区、手势返回、键盘避让和至少 44pt/dp 的触控区域；桌面考虑窗口缩放、快捷键与菜单；颜色、文本大小与减少动态效果尊重系统设置。iOS 与 Android 预览主要展示导航与系统边距，实际控件、无障碍和手势需原生验证。

## 复用与迁移

| 现有部分 | Rust 客户端处理 |
| --- | --- |
| `data/word-atlas.json`、`public/atlas/index.json`、词条分片 | 复用内容与 ID；使用 Rust 数据模型解析，建立本地索引 |
| WebP 插画、MP3 发音与例句 | 复用内容；验证 WebP/MP3 解码、缓存和打断恢复；保留 CC BY 4.0 署名 |
| `src/store/srs.ts`、`session.ts`、`lib/typing.ts` | 按行为移植为 Rust；不直接调用 TS；复用已有测试场景与固定样例作为比对基准 |
| React 页面、Hooks、CSS | 用 NSH Rust 组件、响应式 signal、类型化样式重写 |
| `localStorage`、浏览器生命周期 | SQLite 事务存储、版本化 schema、旧 JSON 首次迁移与兼容导出；应用生命周期仍需逐平台验证 |
| `HTMLAudioElement`、Web Audio、speechSynthesis | Rust 音频服务；先验证 NSH/平台音频能力，再决定是否引入 Rust 音频库或平台桥接 |
| WebGL2 星图与 GLSL | 先做可访问的 2D 话题星图，再评估 Rust Canvas / GPU 绘图；不承诺复用 WebGL 代码 |
| Service Worker / 浏览器缓存 | 随安装包内置完整资源；统一资源清单、完整性检查与版本更新 |
| JSON 进度备份 | 导入兼容层：保留词条 ID、毫秒时间戳、时区语义与卡片字段；导入前预览，事务提交 |

媒体文件目前在本机 `du -sh public/atlas` 约占 **708 MB**（文件系统占用，并非最终下载包体）。按用户要求，首版将完整词库、插画、英音、美音和例句音频全部作为安装包资源随应用分发。资源随包携带，不要求编成 Rust 字节常量；资源路径由平台资源定位服务提供。首次安装不请求下载，升级时跟随完整应用更新。全量资源是首版约束，本轮不设计下载中心或可选资源包。需要实测压缩后包体、安装峰值空间、资源读取和各平台分发限制。

## Rust 代码结构建议

`native/` 已建立独立 Cargo workspace，并使用 `Cargo.lock` 锁定依赖。现有网页继续维护，模块结构如下：

```text
native/
  Cargo.toml
  Cargo.lock
  nsh.json                   # rust + root，使用 Rust 模板
  crates/
    atlas-domain/            # 词条、卡片状态、会话、FSRS、拼写判定
    atlas-storage/           # 本地持久化、迁移、备份导入导出
    atlas-media/             # 内置资源定位、音频接口与生命周期
    atlas-ui/                # NSH Rust 根组件、主题与页面
      src/
        shell/               # 桌面侧栏 / 手机导航
        screens/             # 今日、图鉴、学习、星图、我的
        components/          # 插画词卡、发音按钮、自评按钮等
```

领域层不依赖 NSH，便于独立测试。UI 共享组件与状态，外壳按窗口尺寸和平台适配。存储与音频通过 Rust 接口隔离平台差异。

FSRS 已按本仓库锁定的 `ts-fsrs@5.4.2` 移植为 Rust，保留 FSRS-6 权重、短期学习步、UTC 日差和 seeded fuzz。450 组固定历史、1,800 个候选评分结果与网页版逐字段对照通过。例句填空也已对全部 14,596 条例句校验匹配结果。客户端运行时不调用 TypeScript 调度器。

## 实施顺序与验收

1. **纯 Rust 验证**：锁定 NSH 发布版本或提交；运行官方 Rust 模板，在 macOS、iOS 模拟器、Android 模拟器验证中文、WebP、MP3、文件持久化与恢复。确认目标设备最低系统符合预期。先验证主链路，再逐步推进完整迁移。
2. **学习闭环**：今日计划 → 单词插画/释义 → 发音 → 回忆 → 自评 → FSRS → 保存 → 重新打开恢复。覆盖英文/中文输入、手机返回与键盘、桌面窗口缩放。
3. **全量资源与进度迁移**：全量资源随包校验、安装空间与资源路径验证、飞行模式首次启动和重启学习；导入网页备份，保留旧文件，校验卡片与时间数据。
4. **图鉴与练习**：7,298 个词条搜索与列表虚拟化、分片加载、英美音切换、打字练习、设置与无障碍。
5. **星图与分发**：2D Rust 星图先可用，再用真机帧率/内存数据决定 GPU 特效；在 Windows、Linux 补齐构建和交互验证；分别处理签名、安装器与商店交付。

NSH 的 [Canvas 指南](https://nsh.dev/docs/canvas/) 说明 Rust 可以通过 `Painter` 录制绘图，由各平台原生回放。它证明存在实现 2D 星图的路线，不证明现有 7,298 点 WebGL 星图可直接迁移或达到同样性能。

主要待验证项：Rust API 的版本稳定性、音频跨平台行为、媒体解码、长列表与绘图性能、应用后台保存、手机文件导入、签名发布。官方[兼容性文档](https://nsh.dev/docs/compatibility/)说明仅 `main` 上尚未发布的 API 仍可能改变，因此应锁定版本，不直接追随分支。

## 本次验证

- `npm test`：21 项通过。
- `npm run check:atlas`：0 个错误；保留现有的 24 条例句 headword 提示。
- `npm run build`：通过。
- 设计预览：三种设计 × 三种设备 × 浅/深两种主题 × 三个窗口宽度（含 375px 内容宽度与 320px 内容宽度），共 54 个浏览器场景；检查横向溢出、显示释义、自评切换样例词与返回，均通过。
- Rust：43 项有效测试通过；另有一个由子进程崩溃测试主动调用的辅助用例，默认不单独运行。
- `cargo fmt --check` 与 workspace / all-targets / all-features 的 Clippy 检查通过。
- SQLite 版 macOS 和 iOS 模拟器 `.app` 均已重新构建。两包各 37,633 个资源的 SHA-256 比对通过，包括全部插画和 22,905 段录音。
- iOS 26.5 独立模拟器：已安装并启动 SQLite 版；实际迁移旧 JSON、保留卡片/收藏、完整性检查、重启以及忽略陈旧 JSON 的检查通过。
- macOS 原生窗口此前已打开；本轮 SQLite 版经过原生根组件测试并重新打包。
- Android SDK/NDK 按用户决定暂不安装；Android 安装包、Windows/Linux 实机、iOS 真机与媒体/性能专项验收仍未完成。
