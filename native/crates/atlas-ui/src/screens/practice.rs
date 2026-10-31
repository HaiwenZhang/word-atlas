//! Word, sentence and dictation practice with cumulative edit-based accuracy and WPM.

use atlas_domain::{model::TypingRound, typing};
use nsh_base::input;
use nsh_core::prelude::*;

use crate::{
    state::{self, AppState},
    theme::*,
};

#[derive(Default, Clone, PartialEq)]
struct Round {
    index: usize,
    value: String,
    correct: u32,
    attempted: u32,
    accepted: u32,
    started: Option<i64>,
    finished: bool,
    message: String,
}

pub(crate) fn practice(state: AppState) -> AnyElement {
    let mode = signal("word");
    let round = signal(Round::default());
    let pool = state.with(|s| {
        let all = s.catalog.entries();
        let start = typing::hash_seed(&state::day()) as usize % all.len();
        std::sync::Arc::new(
            (0..20)
                .map(|i| all[(start + i * 17) % all.len()].clone())
                .collect::<Vec<_>>(),
        )
    });
    let done = computed(move || round.with(|r| r.finished));
    let for_target = pool.clone();
    let target = computed(move || {
        round.with(|r| {
            let entry = &for_target[r.index.min(for_target.len() - 1)];
            if mode.get() == "sentence" {
                entry
                    .ex
                    .first()
                    .map_or_else(|| entry.w.clone(), |e| e.en.clone())
            } else {
                entry.w.clone()
            }
        })
    });
    let for_word = pool.clone();
    let word =
        computed(move || round.with(|r| for_word[r.index.min(for_word.len() - 1)].id.clone()));
    let for_meaning = pool.clone();
    let meaning = computed(move || {
        round.with(|r| {
            for_meaning[r.index.min(for_meaning.len() - 1)]
                .senses
                .first()
                .map_or_else(String::new, |s| s.zh.clone())
        })
    });
    let edit = move |value: &str| {
        let expected = target.get();
        round.update(|r| {
            if r.finished {
                return;
            }
            if r.started.is_none() {
                r.started = Some(state::now());
            }
            let delta = typing::delta(&r.value, value, &expected);
            r.correct += delta.correct;
            r.attempted += delta.total;
            r.value = value.into();
            r.message.clear();
        });
    };
    let submit = move || {
        let expected = target.get();
        let accepted = round.with(|r| !r.finished && typing::same(&r.value, &expected));
        if !accepted {
            round.update(|r| r.message = "再检查一下拼写、空格和标点。".into());
            return;
        }
        let last = round.with(|r| r.index + 1 >= 20);
        if last {
            let result = round.with(|r| {
                let at = state::now();
                let elapsed = at.saturating_sub(r.started.unwrap_or(at)) as u64;
                let metrics = typing::metrics(
                    r.correct,
                    r.attempted,
                    r.accepted + typing::clean(&expected).chars().count() as u32,
                    elapsed,
                );
                TypingRound {
                    at,
                    mode: mode.get().into(),
                    items: 20,
                    wpm: metrics.wpm.min(400),
                    accuracy: metrics.accuracy,
                }
            });
            let mut committed = false;
            state.update(|s| {
                committed = s.update(|_, snapshot| {
                    snapshot.progress.typing.push(result);
                    let length = snapshot.progress.typing.len();
                    if length > 500 {
                        snapshot.progress.typing.drain(..length - 500);
                    }
                    Ok(())
                })
            });
            if !committed {
                return;
            }
        }
        round.update(|r| {
            r.accepted += typing::clean(&expected).chars().count() as u32;
            r.value.clear();
            r.message.clear();
            if last {
                r.finished = true;
            } else {
                r.index += 1;
            }
        });
        if !last && mode.get() == "dictation" {
            let id = word.get();
            state.update(|s| s.play_word(&id, 1.0));
        }
    };
    let current_metrics = move || {
        round.with(|r| {
            typing::metrics(
                r.correct,
                r.attempted,
                r.accepted,
                r.started
                    .map_or(0, |start| state::now().saturating_sub(start) as u64),
            )
        })
    };
    scroll().flex_1().child(view().p(move||px(if wide(){32.0}else{20.0})).gap_6().max_w(px(1000.0)).w_full().self_center()
        .child(view().gap_2().child(eyebrow("FOCUS PRACTICE")).child(heading("把词语，留在指尖。")))
        .child(view().flex_row().flex_wrap().gap_3().children([("word","单词"),("sentence","例句"),("dictation","听写")].into_iter().map(move|(key,title)|secondary(title,move||{
            mode.set(key);round.set(Round::default());
            if key=="dictation" {let id=word.get();state.update(|s|s.play_word(&id,1.0));}
        }))))
        .child(panel().gap_5()
            .child(muted(move||round.with(|r|format!("{} / 20 · {}% 准确率 · {} WPM",(r.index+1).min(20),current_metrics().accuracy,current_metrics().wpm))))
            .child(dynamic(move|| {
                let finished=done.get();
                if finished {return view().gap_5().child(heading("练习完成。"))
                    .child(label("成绩已保存在本机。"))
                    .child(secondary("再练一轮",move||round.set(Round::default()))).into_element();}
                untracked(||view().gap_5()
                    .child(when(move||mode.get()!="dictation",move||text(move||target.get()).font_family("Georgia").text_size(px(28.0)).text_color(INK)))
                    .child(label(move||meaning.get()))
                    .child(primary("▶ 听发音",move||{let id=word.get();state.update(|s|s.play_word(&id,1.0));}))
                    .child(input().label("练习输入").placeholder("在这里输入，Enter 确认").value(move||round.with(|r|r.value.clone())).on_change(edit).on_submit(move|_:&str|submit())
                        .auto_correct(false).auto_focus(true).min_h(px(54.0)).px_4().bg(PAPER).border_1().border_color(LINE).rounded(px(10.0)).text_size(px(21.0)).text_color(INK))
                    .child(muted(move||round.with(|r|r.message.clone())))
                    .child(primary("确认，下一项  →",submit)).into_element())
            })))
        .child(muted("每次插入和修改都计入准确率；删除不会抹去已发生的错误。WPM 按每 5 个字符计一个词。")))
        .into_element()
}
