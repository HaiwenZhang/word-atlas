//! Local statistics, settings, explicit backup replacement and source attribution.

use atlas_domain::model::{Progress, Settings};
use nsh_base::{alert_dialog, input};
use nsh_core::prelude::*;

use crate::{
    state::{AppState, Page},
    theme::*,
};

fn setting(
    state: AppState,
    title: &'static str,
    values: Vec<(String, String)>,
    read: impl Fn(&Settings) -> String + 'static,
    write: impl Fn(&mut Settings, &str) + 'static,
) -> AnyElement {
    let write = std::rc::Rc::new(write);
    let read = std::rc::Rc::new(read);
    panel()
        .child(label(title))
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .children(values.into_iter().map(move |(value, display)| {
                    let write = write.clone();
                    let read = read.clone();
                    let selected = value.clone();
                    nsh_base::button()
                        .accessibility_label(Some(format!("{title}：{display}").into()))
                        .min_h(px(44.0))
                        .px_4()
                        .py_2()
                        .rounded(px(9.0))
                        .border_1()
                        .border_color(LINE)
                        .bg(move || {
                            state.with(|s| {
                                if read(&s.snapshot.settings) == selected {
                                    SOFT
                                } else {
                                    PAPER
                                }
                            })
                        })
                        .on_press(move || {
                            state.update(|s| {
                                s.update(|_, snapshot| {
                                    write(&mut snapshot.settings, &value);
                                    Ok(())
                                });
                            })
                        })
                        .child(crate::theme::label(display))
                })),
        )
        .into_element()
}

fn toggle(
    state: AppState,
    title: &'static str,
    read: impl Fn(&Settings) -> bool + Copy + 'static,
    write: impl Fn(&mut Settings, bool) + Copy + 'static,
) -> AnyElement {
    view()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_4()
        .child(label(title))
        .child(dynamic(move || {
            let enabled = state.with(|s| read(&s.snapshot.settings));
            secondary(
                if enabled {
                    format!("{title}：已开启")
                } else {
                    format!("{title}：已关闭")
                },
                move || {
                    state.update(|s| {
                        s.update(|_, snapshot| {
                            write(&mut snapshot.settings, !enabled);
                            Ok(())
                        });
                    })
                },
            )
        }))
        .into_element()
}

pub(crate) fn onboarding(state: AppState) -> AnyElement {
    scroll().flex_1().child(view().p_6().max_w(px(720.0)).w_full().self_center().gap_6()
        .child(image("asset:/brand/hero.webp").h(px(220.0)).w_full().resize_mode(ResizeMode::Cover).rounded(px(18.0)))
        .child(eyebrow("欢迎来到词光"))
        .child(heading("把单词变成画面，\n让复习有迹可循。"))
        .child(label("7,298 个词条，插画、英美发音和例句全部内置。无需注册，学习记录留在自己的设备上。"))
        .child(setting(state,"每日新词",[5,10,15,20,30].into_iter().map(|n|(n.to_string(),format!("{n} 个"))).collect(),|s|s.daily_new.to_string(),|s,value|{if let Ok(n)=value.parse(){s.daily_new=n;}}))
        .child(setting(state,"开始难度",["A1","A2","B1","B2","C1","C2"].into_iter().enumerate().map(|(i,v)|(i.to_string(),v.into())).collect(),|s|s.start_level.to_string(),|s,value|{if let Ok(n)=value.parse(){s.start_level=n;}}))
        .child(primary("开启我的词光之旅  ↗",move||state.update(|s|{s.update(|_,snapshot|{snapshot.settings.onboarded=true;Ok(())});}))))
        .into_element()
}

fn preview_backup(state: AppState, text: &str) {
    let result = state.with(|s| atlas_storage::preview_import(text, &s.catalog));
    state.update(|s| match result {
        Ok(candidate) => {
            s.pending_import = Some(candidate);
            s.error = None;
        }
        Err(error) => s.error = Some(error.to_string()),
    });
}

pub(crate) fn me(state: AppState) -> AnyElement {
    let pasted = signal(String::new());
    let import_preview = computed(move || {
        state.with(|s| {
            s.pending_import.as_ref().map(|snapshot| {
                (
                    snapshot.progress.cards.len(),
                    snapshot.progress.bookmarks.len(),
                )
            })
        })
    });
    scroll().flex_1().child(view().p(move||px(if wide(){32.0}else{20.0})).max_w(px(1000.0)).w_full().self_center().gap_5()
        .child(eyebrow("我的"))
        .child(heading("你的星空，自己珍藏。"))
        .child(panel().child(label(move||state.with(|s|{
            let p=&s.snapshot.progress;let reviews:u32=p.days.values().map(|d|d.r).sum();let correct:u32=p.days.values().map(|d|d.c).sum();let minutes:u32=p.days.values().map(|d|d.t).sum::<u32>()/60;
            format!("已点亮 {} · 长期记住 {}\n首次正确率 {}% · 学习 {} 分钟",p.cards.len(),p.cards.values().filter(|c|c.is_mastered()).count(),if reviews>0{(correct as f64/reviews as f64*100.0).round() as u32}else{0},minutes)
        }))).child(muted("长期记住：记忆稳定度至少 21 天。")))
        .child(setting(state,"每日新词",[0,5,10,15,20,30,50].into_iter().map(|n|(n.to_string(),format!("{n} 个"))).collect(),|s|s.daily_new.to_string(),|s,v|{if let Ok(n)=v.parse(){s.daily_new=n;}}))
        .child(setting(state,"开始难度",["A1","A2","B1","B2","C1","C2"].into_iter().enumerate().map(|(i,v)|(i.to_string(),v.into())).collect(),|s|s.start_level.to_string(),|s,v|{if let Ok(n)=v.parse(){s.start_level=n;}}))
        .child(setting(state,"目标记忆率",[("0.85","85%"),("0.9","90%"),("0.95","95%")].into_iter().map(|(v,l)|(v.into(),l.into())).collect(),|s|s.retention.to_string(),|s,v|{if let Ok(n)=v.parse(){s.retention=n;}}))
        .child(setting(state,"发音",[("uk","英式"),("us","美式")].into_iter().map(|(v,l)|(v.into(),l.into())).collect(),|s|s.accent.clone(),|s,v|s.accent=v.into()))
        .child(setting(state,"外观",[("system","跟随系统"),("light","浅色"),("dark","深色")].into_iter().map(|(v,l)|(v.into(),l.into())).collect(),|s|s.theme.clone(),|s,v|s.theme=v.into()))
        .child(panel().child(toggle(state,"自动播放发音",|s|s.autoplay,|s,v|s.autoplay=v)).child(toggle(state,"拼写题",|s|s.spell,|s,v|s.spell=v)))
        .child(panel().child(toggle(state,"界面音效",|s|s.sfx,|s,v|s.sfx=v)))
        .child(setting(state,"环境声",[("off","关闭"),("stars","星空音乐"),("rain","雨天图书馆"),("sea","清晨海岸"),("forest","林间早晨"),("fire","夜晚壁炉"),("voyage","远航船舱"),("cafe","安静咖啡馆")].into_iter().map(|(v,l)|(v.into(),l.into())).collect(),|s|s.ambience.clone(),|s,v|s.ambience=v.into()))
        .child(setting(state,"环境声音量",[("0.2","低"),("0.45","中"),("0.7","高")].into_iter().map(|(v,l)|(v.into(),l.into())).collect(),|s|s.ambience_volume.to_string(),|s,v|{if let Ok(value)=v.parse(){s.ambience_volume=value;}}))
        .child(panel().gap_4().child(label("备份与迁移"))
            .child(muted("备份可与网页版互相导入。导入前会预览记录数；确认替换后，保留一份迁移前快照。"))
            .child(view().flex_row().flex_wrap().gap_3()
                .child(secondary("复制学习备份",move|| {
                    let backup=state.with(|s|atlas_storage::export(&s.snapshot,&chrono::Local::now().to_rfc3339()));
                    match backup {Ok(text)=>{set_clipboard(text);state.update(|s|s.message=Some("备份已复制，可保存为 JSON 文件。".into()));},Err(error)=>state.update(|s|s.error=Some(error.to_string()))}
                }))
                .child(secondary("从剪贴板导入",move||{spawn(async move {let text=clipboard_string().await;preview_backup(state,&text);});})))
            .child(input().label("粘贴备份 JSON").placeholder("也可以在这里粘贴备份 JSON").value(pasted).on_change(move|text:&str|pasted.set(text.into())).multiline(true).min_h(px(100.0)).px_3().py_3().bg(PAPER).border_1().border_color(LINE).rounded(px(10.0)).text_color(INK).text_size(px(12.0)))
            .child(secondary("预览粘贴的备份",move||preview_backup(state,&pasted.get())))
            .child(dynamic(move||import_preview.get().map(|(cards,bookmarks)|panel().child(label(format!("将导入 {cards} 个词的记忆、{bookmarks} 个收藏。现有学习记录将被替换。")))
                .child(view().flex_row().flex_wrap().gap_3().child(secondary("取消导入",move||state.update(|s|s.pending_import=None))).child(primary("确认替换学习记录",move||state.update(|s| {
                    if let Some(candidate)=s.pending_import.clone() {
                        match atlas_storage::confirm_import(&s.backend,&s.catalog,&mut s.snapshot,candidate) {
                            Ok(())=>{s.pending_import=None;s.message=Some("学习记录已恢复。".into());s.error=None;},Err(error)=>s.error=Some(error.to_string())
                        }
                    }
                })))))))
            .child(secondary("恢复迁移前快照",move||{spawn(async move {
                let answer=alert_dialog().title("恢复迁移前快照？").description("这会替换当前进度。建议先复制当前学习备份。").cancel("取消").action("恢复").show().await;
                if matches!(answer,Ok(Some(value)) if value.as_ref()=="恢复") {
                    state.update(|s|match atlas_storage::recover(&s.backend,&s.catalog){Ok(snapshot)=>{s.snapshot=snapshot;s.message=Some("已恢复迁移前的学习记录。".into());},Err(error)=>s.error=Some(error.to_string())});
                }
            });})))
        .child(secondary("关于词光与内容授权",move||state.update(|s|s.navigate(Page::About))))
        .child(secondary("重置学习进度",move||{spawn(async move {
            let answer=alert_dialog().title("重置全部学习进度？").description("收藏、记忆和练习记录会清空。建议先导出备份。").cancel("取消").destructive("重置").show().await;
            if matches!(answer,Ok(Some(value)) if value.as_ref()=="重置") {state.update(|s|{s.update(|catalog,snapshot|{snapshot.progress=Progress::new(catalog.id());Ok(())});});}
        });}))
        .child(muted("本地优先 · 无账号 · 无追踪 · 无资源下载")))
        .into_element()
}

pub(crate) fn about(state: AppState) -> AnyElement {
    scroll().flex_1().child(view().p_6().max_w(px(900.0)).w_full().self_center().gap_5()
        .child(secondary("← 返回我的",move||state.update(|s|s.navigate(Page::Me))))
        .child(heading("词光 · Word Atlas"))
        .child(label("一词一画，点亮雅思。"))
        .child(muted(format!("原生客户端 {} · NSH 0.3.0 · 纯 Rust",env!("CARGO_PKG_VERSION"))))
        .child(panel().child(label("内容署名")).child(label("Word Atlas contributors（词光）"))
            .child(muted("词库文本、插画和词条录音采用 CC BY 4.0。应用代码采用 MIT。NSH 采用 Apache-2.0；FSRS 调度衍生自 ts-fsrs（MIT）。rusqlite 采用 MIT，SQLite 引擎属于公有领域。相关许可随应用资源提供。")))
        .child(panel().child(label("内容来源与局限")).child(label("释义、例句、插画和语音来自原项目的生成与校验流程，尚未逐条人工审校，可能存在错误。它不是官方雅思词表，也不承诺提分。")))
        .child(label("学习记录仅保存在当前设备。更换设备前，请先导出学习备份。")))
        .into_element()
}
