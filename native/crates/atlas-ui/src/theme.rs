//! Warm paper, ink and terracotta, with native light/dark color adaptation.

use nsh_base::button;
use nsh_core::prelude::*;

const fn tone(light: u32, dark: u32) -> AdaptiveColor {
    AdaptiveColor {
        light: rgb(light).to_hsla(),
        dark: rgb(dark).to_hsla(),
    }
}
pub(crate) const PAPER: AdaptiveColor = tone(0xf6f1e7, 0x171c25);
pub(crate) const PANEL: AdaptiveColor = tone(0xfffcf5, 0x222936);
pub(crate) const INK: AdaptiveColor = tone(0x24312f, 0xf3eadc);
pub(crate) const MUTED: AdaptiveColor = tone(0x73766e, 0xb0b4b5);
pub(crate) const LINE: AdaptiveColor = tone(0xe1ddd2, 0x353d47);
pub(crate) const ACCENT: AdaptiveColor = tone(0xac4e32, 0xe79572);
pub(crate) const SOFT: AdaptiveColor = tone(0xeddfd0, 0x3d3029);
pub(crate) const ON_ACCENT: AdaptiveColor = tone(0xfffaf1, 0x221b16);
pub(crate) const GREEN: AdaptiveColor = tone(0x53734c, 0x9dbb90);

pub(crate) fn label(value: impl IntoReactive<SharedString>) -> Text {
    text(value).text_size(px(14.0)).text_color(INK)
}
pub(crate) fn muted(value: impl IntoReactive<SharedString>) -> Text {
    text(value).text_size(px(12.0)).text_color(MUTED)
}
pub(crate) fn eyebrow(value: impl IntoReactive<SharedString>) -> Text {
    text(value)
        .text_size(px(11.0))
        .letter_spacing(px(1.2))
        .text_color(MUTED)
}
pub(crate) fn heading(value: impl IntoReactive<SharedString>) -> Text {
    text(value)
        .font_family("Georgia")
        .text_size(px(30.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(INK)
}
pub(crate) fn panel() -> View {
    view()
        .p_5()
        .gap_3()
        .bg(PANEL)
        .border_1()
        .border_color(LINE)
        .rounded(px(16.0))
}

pub(crate) fn primary(title: impl Into<SharedString>, action: impl Fn() + 'static) -> AnyElement {
    let title: SharedString = title.into();
    button()
        .accessibility_label(Some(title.clone()))
        .min_h(px(46.0))
        .px_5()
        .py_3()
        .rounded(px(10.0))
        .bg(ACCENT)
        .hover(|s| s.opacity(0.9))
        .active(|s| s.opacity(0.75))
        .on_press(action)
        .child(
            text(title)
                .text_color(ON_ACCENT)
                .text_size(px(14.0))
                .font_weight(FontWeight::MEDIUM),
        )
        .into_element()
}
pub(crate) fn secondary(title: impl Into<SharedString>, action: impl Fn() + 'static) -> AnyElement {
    let title: SharedString = title.into();
    button()
        .accessibility_label(Some(title.clone()))
        .min_h(px(44.0))
        .px_4()
        .py_3()
        .rounded(px(10.0))
        .bg(PANEL)
        .border_1()
        .border_color(LINE)
        .hover(|s| s.bg(SOFT))
        .active(|s| s.opacity(0.7))
        .on_press(action)
        .child(text(title).text_color(INK).text_size(px(13.0)))
        .into_element()
}
pub(crate) fn wide() -> bool {
    window().width.0 >= 760.0
}
