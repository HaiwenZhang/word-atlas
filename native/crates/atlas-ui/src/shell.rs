//! Desktop sidebar and mobile navigation around the same persistent application state.

use nsh_base::button;
use nsh_core::prelude::*;

use crate::{
    screens,
    state::{AppState, Page},
    theme::*,
};

const NAV: [(Page, &str, &str); 5] = [
    (Page::Today, "☀", "今日"),
    (Page::Atlas, "▤", "单词图鉴"),
    (Page::Galaxy, "✦", "词汇星图"),
    (Page::Practice, "⌨", "打字练习"),
    (Page::Me, "◯", "我的"),
];

fn brand() -> AnyElement {
    view()
        .flex_row()
        .items_center()
        .gap_3()
        .child(
            view()
                .size(px(36.0))
                .rounded(px(10.0))
                .bg(ACCENT)
                .items_center()
                .justify_center()
                .child(
                    text("词")
                        .font_family("Songti SC")
                        .text_size(px(24.0))
                        .text_color(ON_ACCENT),
                ),
        )
        .child(
            view()
                .gap_1()
                .child(
                    label("词光")
                        .text_size(px(20.0))
                        .font_weight(FontWeight::SEMIBOLD),
                )
                .child(eyebrow("WORD ATLAS").text_size(px(9.0))),
        )
        .into_element()
}

fn nav_item(
    state: AppState,
    page: Page,
    icon: &'static str,
    title: &'static str,
    mobile: bool,
) -> AnyElement {
    let active =
        move || state.with(|s| s.page == page || (page == Page::Today && s.page == Page::Learn));
    button()
        .accessibility_label(Some(title.into()))
        .min_h(px(48.0))
        .px_3()
        .py_2()
        .rounded(px(10.0))
        .bg(move || if active() { SOFT } else { PAPER })
        .selected(active)
        .on_press(move || state.update(|s| s.navigate(page)))
        .child(
            view()
                .style_when(move || !mobile, |s| s.flex_row())
                .items_center()
                .gap_2()
                .child(
                    text(icon)
                        .text_size(px(19.0))
                        .text_color(move || if active() { ACCENT } else { MUTED }),
                )
                .child(
                    text(title)
                        .text_size(px(if mobile { 10.0 } else { 13.0 }))
                        .text_color(move || if active() { ACCENT } else { MUTED }),
                ),
        )
        .into_element()
}

fn sidebar(state: AppState) -> AnyElement {
    view()
        .w(px(188.0))
        .bg(PAPER)
        .border_r_1()
        .border_r_color(LINE)
        .px_4()
        .py_6()
        .gap_2()
        .child(view().px_2().pb_6().child(brand()))
        .child(eyebrow("我的学习").px_3().pb_3())
        .children(
            NAV.into_iter()
                .map(|(page, icon, title)| nav_item(state, page, icon, title, false)),
        )
        .child(view().flex_1())
        .child(
            view()
                .gap_2()
                .px_3()
                .py_4()
                .child(muted("● 全部资源已内置").text_color(GREEN))
                .child(muted("离线也能走得很远").text_size(px(11.0))),
        )
        .child(
            view()
                .border_t_1()
                .border_t_color(LINE)
                .pt_4()
                .px_2()
                .gap_1()
                .child(label("探索者"))
                .child(muted("本地学习档案")),
        )
        .into_element()
}

fn notifications(state: AppState) -> AnyElement {
    let error = computed(move || state.with(|s| s.error.clone()));
    let message = computed(move || state.with(|s| s.message.clone()));
    view()
        .child(dynamic(move || {
            error.get().map(|error| {
                view()
                    .bg(SOFT)
                    .p_3()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(label(error).text_color(ACCENT).flex_1())
                    .child(secondary("关闭提示", move || {
                        state.update(|s| s.error = None)
                    }))
            })
        }))
        .child(dynamic(move || {
            message.get().map(|message| {
                view()
                    .bg(PANEL)
                    .p_3()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .child(muted(message).flex_1())
                    .child(secondary("知道了", move || {
                        state.update(|s| s.message = None)
                    }))
            })
        }))
        .into_element()
}

pub(crate) fn app(state: AppState) -> AnyElement {
    let route = computed(move || state.with(|s| (s.page, s.snapshot.settings.onboarded)));
    let detail = computed(move || state.with(|s| s.detail.clone()));
    let content = move || {
        let (page, onboarded) = route.get();
        untracked(|| {
            if !onboarded {
                return screens::onboarding(state);
            }
            match page {
                Page::Today => screens::today(state),
                Page::Atlas => screens::atlas(state),
                Page::Learn => screens::learn(state),
                Page::Practice => screens::practice(state),
                Page::Galaxy => screens::galaxy(state),
                Page::Me => screens::me(state),
                Page::About => screens::about(state),
            }
        })
    };
    view()
        .flex_1()
        .bg(PAPER)
        .text_color(INK)
        .p_safe_area()
        .child(
            view()
                .flex_1()
                .flex_row()
                .child(when(wide, move || sidebar(state)))
                .child(
                    view()
                        .flex_1()
                        .min_w_0()
                        .child(notifications(state))
                        .child(
                            view()
                                .flex_1()
                                .style_when(
                                    move || detail.with(Option::is_some),
                                    |style| style.hidden(),
                                )
                                .child(dynamic(content)),
                        )
                        .child(dynamic(move || {
                            detail
                                .get()
                                .map(|id| untracked(|| screens::word_detail(state, &id)))
                        })),
                ),
        )
        .child(when(
            move || !wide(),
            move || {
                view()
                    .flex_row()
                    .justify_around()
                    .gap_1()
                    .px_2()
                    .py_2()
                    .border_t_1()
                    .border_t_color(LINE)
                    .bg(PAPER)
                    .children(
                        NAV.into_iter()
                            .filter(|(page, _, _)| *page != Page::Practice)
                            .map(|(page, icon, title)| {
                                nav_item(
                                    state,
                                    page,
                                    icon,
                                    if page == Page::Atlas {
                                        "图鉴"
                                    } else if page == Page::Galaxy {
                                        "星图"
                                    } else {
                                        title
                                    },
                                    true,
                                )
                            }),
                    )
            },
        ))
        .into_element()
}
