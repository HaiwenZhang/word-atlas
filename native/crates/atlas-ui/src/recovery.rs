//! A failed profile load never creates an empty saved profile; recovery is explicit and previewed.

use atlas_storage::{Snapshot, SqliteBackend};
use nsh_core::prelude::*;

use crate::{state, theme::*};

pub(crate) fn screen(error: String, retry: Signal<u32>) -> AnyElement {
    let candidate = signal(None::<Snapshot>);
    let message = signal(error);
    scroll().flex_1().child(view().p_safe_area().p_6().gap_5().max_w(px(760.0)).w_full().self_center().bg(PAPER)
        .child(heading("学习档案尚未打开"))
        .child(label(move||message.get()).text_color(ACCENT))
        .child(muted("原文件已保留。请检查磁盘空间、目录权限，或关闭另一个使用此档案的应用进程。也可以从已有的词光学习备份恢复。"))
        .child(secondary("重新尝试打开",move||retry.update(|n|*n+=1)))
        .child(secondary("从剪贴板预览恢复备份",move||{spawn(async move {
            let text=clipboard_string().await;
            let parsed=atlas_domain::Catalog::bundled().map_err(|e|e.to_string()).and_then(|catalog|atlas_storage::preview_import(&text,&catalog).map_err(|e|e.to_string()));
            match parsed {Ok(snapshot)=>candidate.set(Some(snapshot)),Err(error)=>message.set(error)}
        });}))
        .child(dynamic(move||candidate.get().map(|snapshot|panel()
            .child(label(format!("将恢复 {} 个词的记忆和 {} 个收藏。",snapshot.progress.cards.len(),snapshot.progress.bookmarks.len())))
            .child(muted("只有确认后才写入。原始档案会保留在数据库恢复区；旧 JSON 文件也会保留；恢复失败不会主动清空原文件。"))
            .child(secondary("取消恢复",move||candidate.set(None)))
            .child(primary("确认从备份恢复",move|| {
                let result=state::directory().and_then(|path|SqliteBackend::open_for_repair(path).map_err(|e|e.to_string())).and_then(|backend| {
                    let catalog=atlas_domain::Catalog::bundled().map_err(|e|e.to_string())?;
                    atlas_storage::repair_import(&backend,&catalog,snapshot.clone()).map_err(|e|e.to_string())
                });
                match result {Ok(_)=>retry.update(|n|*n+=1),Err(error)=>message.set(error)}
            })))))
        .child(muted("恢复不会联网或创建账号。")))
        .into_element()
}
