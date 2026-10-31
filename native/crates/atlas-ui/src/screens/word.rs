//! Full dictionary card with installed illustrations, recordings and persistent bookmarks.

use atlas_domain::Entry;
use atlas_media::AudioKind;
use nsh_core::prelude::*;

use crate::{state::AppState, theme::*};

pub(crate) fn entry_content(state: AppState, entry: &Entry) -> AnyElement {
    let id = entry.id.clone();
    let speech = id.clone();
    let british = id.clone();
    let american = id.clone();
    let slow = id.clone();
    let example = id.clone();
    let example_two = id.clone();
    view()
        .gap_5()
        .child(
            view()
                .style_when(wide, |s| s.flex_row())
                .gap_6()
                .child(
                    image(format!("asset:/atlas/img/{}.webp", entry.id))
                        .w(move || if wide() { px(280.0) } else { px(230.0) })
                        .h(move || if wide() { px(280.0) } else { px(230.0) })
                        .self_center()
                        .rounded(px(18.0))
                        .resize_mode(ResizeMode::Cover),
                )
                .child(
                    view()
                        .flex_1()
                        .gap_3()
                        .child(eyebrow(format!("{} · {}", entry.cefr, entry.topic)))
                        .child(heading(entry.w.clone()).text_size(px(44.0)))
                        .child(muted(format!("英 {}   美 {}", entry.uk, entry.us)))
                        .child(
                            view()
                                .flex_row()
                                .flex_wrap()
                                .gap_2()
                                .child(primary("听发音", move || {
                                    state.update(|s| s.play_word(&speech, 1.0))
                                }))
                                .child(secondary("慢速 0.7×", move || {
                                    state.update(|s| s.play_word(&slow, 0.7))
                                }))
                                .child(secondary("英式发音", move || {
                                    state.update(|s| s.play(&british, AudioKind::Uk, 1.0))
                                }))
                                .child(secondary("美式发音", move || {
                                    state.update(|s| s.play(&american, AudioKind::Us, 1.0))
                                })),
                        )
                        .children(entry.senses.iter().map(|sense| {
                            view()
                                .gap_1()
                                .child(
                                    label(format!("{}  {}", sense.pos, sense.zh))
                                        .text_size(px(17.0)),
                                )
                                .child(muted(sense.en.clone()))
                        }))
                        .child(when(
                            {
                                let id = id.clone();
                                move || state.with(|s| s.snapshot.progress.cards.contains_key(&id))
                            },
                            {
                                let id = id.clone();
                                move || {
                                    muted(state.with(|s| {
                                        s.snapshot.progress.cards.get(&id).map_or_else(
                                            String::new,
                                            |c| {
                                                format!("复习 {} 次 · 稳定度 {:.1} 天", c.reps, c.s)
                                            },
                                        )
                                    }))
                                }
                            },
                        )),
                ),
        )
        .children(
            entry
                .ex
                .iter()
                .enumerate()
                .map(|(index, example_sentence)| {
                    let id = if index == 0 {
                        example.clone()
                    } else {
                        example_two.clone()
                    };
                    let has_audio = entry.m & if index == 0 { 8 } else { 16 } != 0;
                    panel()
                        .child(eyebrow(if index == 0 {
                            "日常语境"
                        } else {
                            "雅思语境"
                        }))
                        .child(highlighted(&example_sentence.en, &entry.w))
                        .child(muted(example_sentence.zh.clone()))
                        .child(has_audio.then(|| {
                            secondary(
                                if index == 0 {
                                    "朗读日常例句"
                                } else {
                                    "朗读雅思例句"
                                },
                                move || {
                                    state.update(|s| {
                                        s.play(
                                            &id,
                                            if index == 0 {
                                                AudioKind::Example
                                            } else {
                                                AudioKind::ExampleTwo
                                            },
                                            1.0,
                                        )
                                    })
                                },
                            )
                        }))
                }),
        )
        .child((!entry.col.is_empty()).then(|| {
            panel()
                .child(eyebrow("常用搭配"))
                .children(entry.col.iter().map(|value| label(value.clone())))
        }))
        .child((!entry.fam.is_empty()).then(|| {
            panel()
                .child(eyebrow("词族"))
                .child(label(entry.fam.join(" · ")))
        }))
        .child((!entry.syn.is_empty()).then(|| {
            panel()
                .child(eyebrow("近义词"))
                .child(label(entry.syn.join(" · ")))
        }))
        .child((!entry.ant.is_empty()).then(|| {
            panel()
                .child(eyebrow("反义词"))
                .child(label(entry.ant.join(" · ")))
        }))
        .child((!entry.tip.is_empty()).then(|| {
            panel()
                .child(eyebrow("记忆提示"))
                .child(label(entry.tip.clone()))
        }))
        .child((!entry.note.is_empty()).then(|| label(entry.note.clone()).text_color(ACCENT)))
        .into_element()
}

fn highlighted(sentence: &str, word: &str) -> Text {
    let base = text("")
        .font_family("Georgia")
        .text_size(px(20.0))
        .text_color(INK);
    if let Some(gap) = atlas_domain::text::gap(sentence, word) {
        base.child(text(gap.before.to_owned()))
            .child(
                text(gap.hidden.to_owned())
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(ACCENT),
            )
            .child(text(gap.after.to_owned()))
    } else {
        base.child(text(sentence.to_owned()))
    }
}

pub(crate) fn word_detail(state: AppState, id: &str) -> AnyElement {
    let entry = state.with(|s| s.catalog.entry(id).cloned());
    let Some(entry) = entry else {
        return label("词条不存在").into_element();
    };
    let bookmark = entry.id.clone();
    let practice = entry.id.clone();
    scroll()
        .flex_1()
        .child(
            view()
                .p(move || px(if wide() { 32.0 } else { 20.0 }))
                .gap_5()
                .max_w(px(1000.0))
                .w_full()
                .self_center()
                .child(
                    view()
                        .flex_row()
                        .justify_between()
                        .gap_3()
                        .child(secondary("← 返回", move || {
                            state.update(|s| {
                                s.detail = None;
                                s.audio = None;
                            })
                        }))
                        .child(dynamic(move || {
                            let id = bookmark.clone();
                            let marked =
                                state.with(|s| s.snapshot.progress.bookmarks.contains(&id));
                            secondary(
                                if marked {
                                    "★ 已收藏"
                                } else {
                                    "☆ 收藏"
                                },
                                move || {
                                    state.update(|s| {
                                        s.update(|_, snapshot| {
                                            snapshot.progress.toggle_bookmark(&id);
                                            Ok(())
                                        });
                                    })
                                },
                            )
                        })),
                )
                .child(entry_content(state, &entry))
                .child(primary("练习这个词", move || {
                    state.update(|s| s.start(false, Some(vec![practice.clone()])))
                })),
        )
        .into_element()
}
