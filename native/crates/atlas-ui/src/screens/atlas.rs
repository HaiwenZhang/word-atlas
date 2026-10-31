//! Searchable, filtered, virtualized access to the complete offline vocabulary.

use nsh_base::{button, input};
use nsh_core::prelude::*;

use crate::{state::AppState, theme::*};

pub(crate) fn atlas(state: AppState) -> AnyElement {
    let query = signal(String::new());
    let topic = signal(None::<String>);
    let level = signal(None::<String>);
    let bookmarked = signal(false);
    let ids = computed(move || {
        state.with(|s| {
            s.catalog
                .search(&query.get(), topic.get().as_deref(), level.get().as_deref())
                .into_iter()
                .filter(|entry| {
                    !bookmarked.get() || s.snapshot.progress.bookmarks.contains(&entry.id)
                })
                .map(|e| e.id.clone())
                .collect::<Vec<_>>()
        })
    });
    let topics = state.with(|s| s.catalog.topics().to_vec());
    let levels = state.with(|s| s.catalog.levels().to_vec());
    let topic_picker = signal(false);
    view()
        .flex_1()
        .gap_4()
        .p(move || px(if wide() { 28.0 } else { 16.0 }))
        .child(
            view()
                .gap_1()
                .child(eyebrow("WORD ATLAS"))
                .child(heading("一本可以听的单词图鉴")),
        )
        .child(
            input()
                .label("搜索单词或中文释义")
                .placeholder("搜索单词或中文释义")
                .value(query)
                .on_change(move |value: &str| query.set(value.into()))
                .min_h(px(48.0))
                .px_4()
                .rounded(px(12.0))
                .bg(PANEL)
                .border_1()
                .border_color(LINE)
                .text_color(INK),
        )
        .child(
            view()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .child(secondary("全部话题", move || {
                    topic.set(None);
                    topic_picker.set(false);
                }))
                .child(secondary("选择话题", move || {
                    topic_picker.update(|v| *v = !*v)
                }))
                .child(dynamic(move || {
                    secondary(
                        if bookmarked.get() {
                            "★ 仅看收藏"
                        } else {
                            "☆ 我的收藏"
                        },
                        move || bookmarked.update(|v| *v = !*v),
                    )
                }))
                .child(secondary("全部难度", move || level.set(None)))
                .children(levels.into_iter().map(move |name| {
                    let selected = name.clone();
                    let display = name.clone();
                    button()
                        .accessibility_label(Some(name.clone().into()))
                        .min_h(px(44.0))
                        .px_3()
                        .rounded(px(9.0))
                        .bg(move || {
                            if level.with(|v| v.as_deref() == Some(&selected)) {
                                SOFT
                            } else {
                                PANEL
                            }
                        })
                        .on_press(move || level.set(Some(name.clone())))
                        .child(label(display))
                })),
        )
        .child(when(
            move || topic_picker.get(),
            move || {
                view()
                    .flex_row()
                    .flex_wrap()
                    .gap_2()
                    .children(topics.iter().map(|(id, name)| {
                        let id = id.clone();
                        secondary(name.clone(), move || {
                            topic.set(Some(id.clone()));
                            topic_picker.set(false);
                        })
                    }))
            },
        ))
        .child(muted(move || {
            format!(
                "{} 个词条{}",
                ids.with(Vec::len),
                topic.with(|v| v.as_ref().map_or_else(String::new, |id| format!(" · {id}")))
            )
        }))
        .child(when(
            move || ids.with(Vec::is_empty),
            move || {
                panel()
                    .child(label("没有找到匹配的词。"))
                    .child(muted("换一个关键词，或清除话题、难度和收藏筛选。"))
            },
        ))
        .child(
            virtual_list(
                move || ids.with(Vec::len),
                move |at| {
                    let id =
                        move || ids.with(|ids| ids.get(at.index()).cloned().unwrap_or_default());
                    // Core press targets keep reactive labels with the recycled row owner.
                    // NSH 0.3 base Button props can outlive that owner when filtering a virtual list.
                    view()
                        .role(AccessibilityRole::Button)
                        .cursor_pointer()
                        .label(move || {
                            state.with(|s| {
                                s.catalog
                                    .entry(&id())
                                    .map_or_else(String::new, |e| format!("打开词卡 {}", e.w))
                            })
                        })
                        .h(px(100.0))
                        .px_4()
                        .py_3()
                        .rounded(px(12.0))
                        .bg(PANEL)
                        .border_1()
                        .border_color(LINE)
                        .on_press(move || {
                            let selected = id();
                            state.update(|s| s.open_word(&selected));
                        })
                        .child(
                            view()
                                .flex_row()
                                .items_center()
                                .gap_4()
                                .child(
                                    image(move || format!("asset:/atlas/thumb/{}.webp", id()))
                                        .size(px(68.0))
                                        .rounded(px(8.0)),
                                )
                                .child(
                                    view()
                                        .flex_1()
                                        .min_w_0()
                                        .gap_1()
                                        .child(
                                            text(move || {
                                                state.with(|s| {
                                                    s.catalog
                                                        .entry(&id())
                                                        .map_or_else(String::new, |e| e.w.clone())
                                                })
                                            })
                                            .font_family("Georgia")
                                            .text_size(px(23.0))
                                            .text_color(INK),
                                        )
                                        .child(
                                            muted(move || {
                                                state.with(|s| {
                                                    s.catalog.entry(&id()).map_or_else(
                                                        String::new,
                                                        |e| {
                                                            e.senses
                                                                .first()
                                                                .map_or_else(String::new, |sense| {
                                                                    sense.zh.clone()
                                                                })
                                                        },
                                                    )
                                                })
                                            })
                                            .number_of_lines(1),
                                        ),
                                )
                                .child(muted(move || {
                                    state.with(|s| {
                                        s.catalog
                                            .entry(&id())
                                            .map_or_else(String::new, |e| e.cefr.clone())
                                    })
                                })),
                        )
                },
            )
            .estimated_item_size(px(100.0))
            .row_gap(px(10.0))
            .flex_1(),
        )
        .into_element()
}
