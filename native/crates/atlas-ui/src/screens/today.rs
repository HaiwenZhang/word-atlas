//! Today's real schedule, word of the day and recent learning record.

use atlas_domain::{session, typing::hash_seed};
use chrono::{Datelike, Days, Local};
use nsh_core::prelude::*;

use crate::{
    state::{self, AppState, Page},
    theme::*,
};

pub(crate) fn today(state: AppState) -> AnyElement {
    let plan = computed(move || {
        state.with(|s| {
            session::today_plan(
                &s.catalog,
                &s.snapshot.progress,
                &s.snapshot.settings,
                state::now(),
                &state::day(),
            )
        })
    });
    let active = move || {
        state.with(|s| {
            s.snapshot
                .progress
                .session
                .as_ref()
                .is_some_and(|s| s.finished.is_none())
        })
    };
    let can_start = move || active() || !plan.with(|p| p.due.is_empty() && p.fresh.is_empty());
    let daily = state.with(|s| {
        s.catalog
            .entries()
            .get(hash_seed(&state::day()) as usize % s.catalog.entries().len())
            .cloned()
    });
    scroll()
        .flex_1()
        .child(
            view()
                .p(move || px(if wide() { 32.0 } else { 20.0 }))
                .gap_6()
                .max_w(px(1180.0))
                .w_full()
                .self_center()
                .child(
                    view()
                        .gap_2()
                        .child(eyebrow(format!(
                            "{}月{}日 · 星期{}",
                            Local::now().month(),
                            Local::now().day(),
                            ["日", "一", "二", "三", "四", "五", "六"]
                                [Local::now().weekday().num_days_from_sunday() as usize]
                        )))
                        .child(heading("让每个词，都有画面。")),
                )
                .child(
                    view()
                        .min_h(px(310.0))
                        .bg(PANEL)
                        .rounded(px(18.0))
                        .border_1()
                        .border_color(LINE)
                        .overflow_hidden()
                        .child(
                            image("asset:/brand/hero.webp")
                                .absolute()
                                .right_0()
                                .top_0()
                                .w(percentage(0.48))
                                .h_full()
                                .resize_mode(ResizeMode::Cover)
                                .opacity(move || if wide() { 0.95_f32 } else { 0.5_f32 }),
                        )
                        .child(
                            view()
                                .p_6()
                                .gap_4()
                                .max_w(px(480.0))
                                .child(eyebrow("TODAY’S JOURNEY").text_color(ACCENT))
                                .child(
                                    heading(move || {
                                        if can_start() {
                                            "今天，再点亮\n一点新知。"
                                        } else {
                                            "今天的星，\n都亮了。"
                                        }
                                    })
                                    .text_size(px(32.0)),
                                )
                                .child(
                                    label(move || {
                                        plan.with(|p| {
                                            format!(
                                                "复习 {}   ·   新词 {}",
                                                p.due.len(),
                                                p.fresh.len()
                                            )
                                        })
                                    })
                                    .text_size(px(22.0)),
                                )
                                .child(muted(move || {
                                    plan.with(|p| format!("约 {} 分钟 · 先温故，再知新", p.minutes))
                                }))
                                .child(dynamic(move || {
                                    primary(
                                        if active() {
                                            "继续今天的旅程  ↗"
                                        } else if can_start() {
                                            "开始今天的旅程  ↗"
                                        } else {
                                            "打开单词图鉴  ↗"
                                        },
                                        move || {
                                            let start = can_start();
                                            state.update(|s| {
                                                if start {
                                                    s.start(false, None)
                                                } else {
                                                    s.navigate(Page::Atlas)
                                                }
                                            })
                                        },
                                    )
                                }))
                                .child(when(
                                    move || {
                                        plan.with(|p| !p.due.is_empty() && !p.fresh.is_empty())
                                            && !active()
                                    },
                                    move || {
                                        secondary("只复习", move || {
                                            state.update(|s| s.start(true, None))
                                        })
                                    },
                                )),
                        ),
                )
                .child(
                    view()
                        .style_when(wide, |s| s.flex_row())
                        .gap_4()
                        .child(panel().flex_1().child(eyebrow("每日一词")).child(daily.map(
                            |entry| {
                                let id = entry.id.clone();
                                view()
                                    .flex_row()
                                    .gap_4()
                                    .items_center()
                                    .child(
                                        image(format!("asset:/atlas/thumb/{}.webp", entry.id))
                                            .size(px(88.0))
                                            .rounded(px(10.0)),
                                    )
                                    .child(
                                        view()
                                            .gap_2()
                                            .flex_1()
                                            .child(heading(entry.w).text_size(px(28.0)))
                                            .child(muted(entry.uk))
                                            .child(label(
                                                entry
                                                    .senses
                                                    .first()
                                                    .map_or_else(String::new, |s| s.zh.clone()),
                                            ))
                                            .child(secondary("翻开词卡  ↗", move || {
                                                state.update(|s| s.open_word(&id))
                                            })),
                                    )
                            },
                        )))
                        .child(
                            panel()
                                .flex_1()
                                .child(label("一点一滴，都算数。"))
                                .child(dynamic(move || {
                                    let date = Local::now().date_naive();
                                    view().flex_row().justify_between().children(
                                        (0..7)
                                            .rev()
                                            .filter_map(|offset| {
                                                date.checked_sub_days(Days::new(offset))
                                            })
                                            .map(|date| {
                                                let key = date.format("%Y-%m-%d").to_string();
                                                let count = state.with(|s| {
                                                    s.snapshot
                                                        .progress
                                                        .days
                                                        .get(&key)
                                                        .map_or(0, |day| day.r)
                                                });
                                                let weekday =
                                                    ["日", "一", "二", "三", "四", "五", "六"][date
                                                        .weekday()
                                                        .num_days_from_sunday()
                                                        as usize];
                                                view()
                                                    .items_center()
                                                    .gap_2()
                                                    .child(muted(weekday))
                                                    .child(
                                                        view()
                                                            .size(px(30.0))
                                                            .rounded_full()
                                                            .bg(if count > 0 {
                                                                SOFT
                                                            } else {
                                                                PAPER
                                                            })
                                                            .items_center()
                                                            .justify_center()
                                                            .child(
                                                                label(if count > 0 {
                                                                    "✓"
                                                                } else {
                                                                    "·"
                                                                })
                                                                .text_color(ACCENT),
                                                            ),
                                                    )
                                            }),
                                    )
                                }))
                                .child(label(move || {
                                    state.with(|s| {
                                        format!(
                                            "{} 个词已点亮 · {} 个长期记住",
                                            s.snapshot.progress.cards.len(),
                                            s.snapshot
                                                .progress
                                                .cards
                                                .values()
                                                .filter(|c| c.is_mastered())
                                                .count()
                                        )
                                    })
                                })),
                        ),
                )
                .child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .gap_3()
                        .child(secondary("打字练习", move || {
                            state.update(|s| s.navigate(Page::Practice))
                        }))
                        .child(secondary("看看我的星图", move || {
                            state.update(|s| s.navigate(Page::Galaxy))
                        })),
                )
                .child(muted("一词一画，一点一滴。  ·  7,298 个词条，完整离线")),
        )
        .into_element()
}
