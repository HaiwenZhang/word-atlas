//! A native two-dimensional topic atlas with accessible per-topic word navigation.

use atlas_domain::typing::hash_seed;
use nsh_core::prelude::*;

use crate::{state::AppState, theme::*};

pub(crate) fn galaxy(state: AppState) -> AnyElement {
    let selected = signal(None::<String>);
    let topics = state.with(|s| s.catalog.topics().to_vec());
    let dimensions = signal((300.0_f32, 280.0_f32));
    let canvas = canvas(move || {
        let (width, height) = dimensions.get();
        let ink = color(MUTED);
        let accent = color(ACCENT);
        state.with(|s| {
            let mut paint = Painter::new();
            for (topic_index, (id, _)) in s.catalog.topics().iter().enumerate() {
                let center_x = (topic_index % 5) as f32 * (width / 5.0) + width / 10.0;
                let center_y = (topic_index / 5) as f32 * (height / 4.0) + height / 8.0;
                for entry in s.catalog.entries().iter().filter(|e| &e.topic == id) {
                    let hash = hash_seed(&entry.id);
                    let x = center_x + (hash % 100) as f32 / 100.0 * (width / 6.0) - width / 12.0;
                    let y = center_y + ((hash / 100) % 100) as f32 / 100.0 * (height / 5.0)
                        - height / 10.0;
                    let learned = s.snapshot.progress.cards.contains_key(&entry.id);
                    paint = paint
                        .fill_color(if learned { accent } else { ink.opacity(0.35) })
                        .begin_path()
                        .arc(
                            x,
                            y,
                            if learned { 2.2 } else { 1.0 },
                            0.0,
                            std::f32::consts::TAU,
                        )
                        .fill();
                }
            }
            paint
        })
    })
    .h(px(280.0))
    .w_full()
    .on_layout(move |bounds: &Bounds<Points>| {
        dimensions.set((bounds.size.width.0, bounds.size.height.0))
    })
    .label("19 个话题的词汇星群，使用下方话题按钮查看全部单词");
    scroll()
        .flex_1()
        .child(
            view()
                .p(move || px(if wide() { 32.0 } else { 20.0 }))
                .gap_5()
                .max_w(px(1100.0))
                .w_full()
                .self_center()
                .child(eyebrow("YOUR VOCABULARY UNIVERSE"))
                .child(heading("每学会一个词，就点亮一颗星。"))
                .child(label(move || {
                    state.with(|s| {
                        format!("已点亮 {} / 7,298 颗星", s.snapshot.progress.cards.len())
                    })
                }))
                .child(panel().child(canvas).child(muted(
                    "星群按话题排列；下方列表包含全部词条，支持屏幕阅读器。",
                )))
                .child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .gap_2()
                        .children(topics.into_iter().map(move |(id, name)| {
                            let count = state.with(|s| {
                                s.catalog
                                    .entries()
                                    .iter()
                                    .filter(|e| {
                                        e.topic == id
                                            && s.snapshot.progress.cards.contains_key(&e.id)
                                    })
                                    .count()
                            });
                            secondary(format!("{name} · {count}"), move || {
                                selected.set(Some(id.clone()))
                            })
                        })),
                )
                .child(dynamic(move || {
                    let topic = selected.get();
                    let Some(topic) = topic else {
                        return muted("选择一个话题，打开你的词汇星群。").into_element();
                    };
                    let words = state.with(|s| {
                        s.catalog
                            .entries()
                            .iter()
                            .filter(|entry| entry.topic == topic)
                            .map(|entry| {
                                (
                                    entry.id.clone(),
                                    entry.w.clone(),
                                    s.snapshot.progress.cards.contains_key(&entry.id),
                                )
                            })
                            .collect::<Vec<_>>()
                    });
                    view()
                        .gap_3()
                        .child(label(format!("这个话题有 {} 个词", words.len())))
                        .child(view().flex_row().flex_wrap().gap_2().children(
                            words.into_iter().map(|(id, word, learned)| {
                                secondary(
                                    format!("{} {word}", if learned { "✦" } else { "·" }),
                                    move || state.update(|s| s.open_word(&id)),
                                )
                            }),
                        ))
                        .into_element()
                })),
        )
        .into_element()
}
