//! Introductions, four active-recall formats, feedback and resumable journey summaries.

use atlas_domain::{Grade, model::StepKind, session};
use nsh_base::input;
use nsh_core::prelude::*;

use super::word::entry_content;
use crate::{
    state::{AppState, Page},
    theme::*,
};

pub(crate) fn learn(state: AppState) -> AnyElement {
    let phase = computed(move || {
        state.with(|s| {
            s.snapshot.progress.session.as_ref().map(|j| {
                (
                    j.id.clone(),
                    j.index,
                    j.pending.is_some(),
                    j.finished.is_some(),
                )
            })
        })
    });
    view()
        .flex_1()
        .child(dynamic(move || {
            let _ = phase.get();
            untracked(|| screen(state))
        }))
        .into_element()
}

fn screen(state: AppState) -> AnyElement {
    let journey = state.with(|s| s.snapshot.progress.session.clone());
    let Some(journey) = journey else {
        return panel()
            .child(heading("准备好，再出发。"))
            .child(primary("开始今天的旅程", move || {
                state.update(|s| s.start(false, None))
            }))
            .into_element();
    };
    if journey.finished.is_some() {
        return summary(state, &journey);
    }
    let Some(step) = journey.steps.get(journey.index).cloned() else {
        return label("学习会话没有可用题目。").into_element();
    };
    let entry = state.with(|s| s.catalog.entry(&step.id).cloned());
    let Some(entry) = entry else {
        return label("无法读取当前词条。").into_element();
    };
    let content = if step.kind == StepKind::Intro {
        view()
            .gap_5()
            .child(eyebrow("初次见面"))
            .child(entry_content(state, &entry))
            .child(primary("认识了，继续  →", move || {
                state.update(|s| s.advance())
            }))
            .into_element()
    } else if let Some(pending) = &journey.pending {
        let headline = if pending.correct {
            "答对了，稳稳向前。"
        } else {
            "没关系，再记住它。"
        };
        view()
            .gap_5()
            .child(heading(headline).text_size(px(25.0)))
            .child(entry_content(state, &entry))
            .child(primary("继续  →", move || {
                state.update(|s| s.advance())
            }))
            .into_element()
    } else {
        let mut prompt = view().gap_5().items_center();
        let id = entry.id.clone();
        match step.kind {
            StepKind::Meaning => {
                prompt = prompt
                    .child(eyebrow("它是什么意思？"))
                    .child(heading(entry.w.clone()).text_size(px(48.0)))
                    .child(secondary("听发音", move || {
                        state.update(|s| s.play_word(&id, 1.0))
                    }));
            }
            StepKind::Picture => {
                prompt = prompt
                    .child(eyebrow("看图，补全这句话"))
                    .child(
                        image(format!("asset:/atlas/img/{}.webp", entry.id))
                            .size(px(230.0))
                            .rounded(px(16.0)),
                    )
                    .child(
                        text(
                            entry
                                .ex
                                .first()
                                .and_then(|example| atlas_domain::text::gap(&example.en, &entry.w))
                                .map_or_else(
                                    || {
                                        entry
                                            .senses
                                            .first()
                                            .map_or_else(String::new, |sense| sense.zh.clone())
                                    },
                                    |gap| format!("{}________{}", gap.before, gap.after),
                                ),
                        )
                        .font_family("Georgia")
                        .text_size(px(22.0))
                        .text_color(INK),
                    );
            }
            StepKind::Listen => {
                let slow = id.clone();
                prompt = prompt
                    .child(eyebrow("听，是哪个词？"))
                    .child(primary("▶ 再听一遍", move || {
                        state.update(|s| s.play_word(&id, 1.0))
                    }))
                    .child(secondary("慢速再听", move || {
                        state.update(|s| s.play_word(&slow, 0.7))
                    }));
            }
            StepKind::Spell => {
                prompt = prompt
                    .child(eyebrow("拼出这个词"))
                    .child(
                        image(format!("asset:/atlas/img/{}.webp", entry.id))
                            .size(px(180.0))
                            .rounded(px(16.0)),
                    )
                    .child(label(
                        entry
                            .senses
                            .first()
                            .map_or_else(String::new, |s| s.zh.clone()),
                    ));
            }
            StepKind::Intro => {}
        }
        if step.kind == StepKind::Spell {
            let typed = signal(String::new());
            let hints = signal(0_usize);
            let word = entry.w.clone();
            let hint_word = word.clone();
            prompt = prompt
                .child(
                    input()
                        .label("输入拼写")
                        .placeholder("输入单词")
                        .value(typed)
                        .on_change(move |value: &str| typed.set(value.into()))
                        .on_submit(move |value: &str| {
                            if !value.trim().is_empty() {
                                state.update(|s| s.answer(value, hints.get() > 0));
                            }
                        })
                        .auto_focus(true)
                        .auto_correct(false)
                        .min_h(px(52.0))
                        .w_full()
                        .px_4()
                        .bg(PANEL)
                        .border_1()
                        .border_color(LINE)
                        .rounded(px(12.0))
                        .text_size(px(24.0))
                        .text_color(INK),
                )
                .child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .gap_3()
                        .child(secondary("提示字母", move || {
                            hints.update(|n| {
                                *n = (*n + 1).min(hint_word.chars().count().saturating_sub(1))
                            });
                            typed.set(hint_word.chars().take(hints.get()).collect());
                        }))
                        .child(primary("确认拼写", move || {
                            let value = typed.get();
                            if !value.trim().is_empty() {
                                let hinted = hints.get() > 0;
                                state.update(|s| s.answer(&value, hinted));
                            }
                        })),
                );
        } else {
            let options = step.options.unwrap_or_default();
            prompt = prompt.child(view().gap_3().w_full().children(
                options.into_iter().enumerate().filter_map(|(i, id)| {
                    let option = state.with(|s| s.catalog.entry(&id).cloned())?;
                    let title = if step.kind == StepKind::Meaning {
                        option
                            .senses
                            .first()
                            .map_or_else(String::new, |s| format!("{}.  {}", i + 1, s.zh))
                    } else {
                        format!("{}.  {}", i + 1, option.w)
                    };
                    Some(secondary(title, move || {
                        state.update(|s| s.answer(&id, false))
                    }))
                }),
            ));
        }
        prompt.into_element()
    };
    scroll()
        .flex_1()
        .child(
            view()
                .p(move || px(if wide() { 32.0 } else { 20.0 }))
                .max_w(px(1050.0))
                .w_full()
                .self_center()
                .gap_6()
                .child(
                    view()
                        .flex_row()
                        .justify_between()
                        .items_center()
                        .gap_3()
                        .child(secondary("← 暂停学习", move || {
                            state.update(|s| s.navigate(Page::Today))
                        }))
                        .child(muted(format!(
                            "{} / {} 步",
                            journey.index + 1,
                            journey.steps.len()
                        ))),
                )
                .child(
                    view().w_full().h(px(3.0)).bg(LINE).rounded_full().child(
                        view()
                            .w(percentage(
                                journey.index as f32 / journey.steps.len().max(1) as f32,
                            ))
                            .h_full()
                            .bg(ACCENT),
                    ),
                )
                .child(content),
        )
        .into_element()
}

fn summary(state: AppState, journey: &atlas_domain::model::Session) -> AnyElement {
    let total = journey.graded.len();
    let accuracy = session::accuracy(journey);
    let wrong: Vec<_> = journey
        .graded
        .iter()
        .filter(|(_, grade)| **grade == Grade::Again)
        .map(|(id, _)| id.clone())
        .collect();
    let words: Vec<_> = journey.graded.keys().cloned().collect();
    scroll()
        .flex_1()
        .child(
            view()
                .p_6()
                .gap_6()
                .max_w(px(900.0))
                .w_full()
                .self_center()
                .child(eyebrow("这一程结束了"))
                .child(heading(if accuracy >= 90 {
                    "漂亮的一程。"
                } else if accuracy >= 60 {
                    "稳稳向前。"
                } else {
                    "错过的，会再相遇。"
                }))
                .child(
                    panel()
                        .child(heading(format!("{accuracy}%")).text_color(ACCENT))
                        .child(label(format!("首次正确率 · 共 {total} 个词")))
                        .child(muted(format!(
                            "新词 {} · 复习 {}",
                            journey.new_ids.len(),
                            journey.review_ids.len()
                        ))),
                )
                .child(view().flex_row().flex_wrap().gap_3().children(
                    words.into_iter().filter_map(|id| {
                        let entry = state.with(|s| s.catalog.entry(&id).cloned())?;
                        Some(secondary(entry.w, move || {
                            state.update(|s| s.open_word(&id))
                        }))
                    }),
                ))
                .child(muted("下次复习时间已根据首次作答安排；重练不会重复计分。"))
                .child((!wrong.is_empty()).then(|| {
                    primary(format!("再练 {} 个错词", wrong.len()), move || {
                        state.update(|s| s.start(false, Some(wrong.clone())))
                    })
                }))
                .child(primary("完成，返回今日", move || {
                    state.update(|s| {
                        if s.update(|_, snapshot| {
                            snapshot.progress.session = None;
                            Ok(())
                        }) {
                            s.navigate(Page::Today);
                        }
                    })
                })),
        )
        .into_element()
}
