//! The entirely Rust Word Atlas application, rendered by each NSH platform backend.

use nsh_core::prelude::*;

mod audio;
mod recovery;
mod screens;
mod shell;
mod state;
mod theme;

pub use state::StorageDirectory;

#[native_module("word-atlas/native")]
fn native(registry: &mut Registry) {
    registry.register::<App>();
    registry.assets(asset_files());
}

/// The complete offline resource registry, copied by `nsh package` rather than embedded as bytes.
pub fn asset_files() -> &'static [nsh_core::element::AssetFile] {
    use nsh_core::element::{AssetFile, AssetKind};
    use std::sync::OnceLock;
    static ASSETS: OnceLock<Vec<AssetFile>> = OnceLock::new();
    ASSETS.get_or_init(|| {
        include_str!(concat!(env!("OUT_DIR"), "/asset-manifest.tsv"))
            .lines()
            .filter_map(|line| {
                let (path, source) = line.split_once('\t')?;
                let kind = if path.ends_with(".webp")
                    || path.ends_with(".jpg")
                    || path.ends_with(".png")
                {
                    AssetKind::Image
                } else {
                    AssetKind::File
                };
                Some(AssetFile {
                    uri: Box::leak(format!("asset:/{path}").into_boxed_str()),
                    source,
                    kind,
                })
            })
            .collect()
    })
}

/// The root shared by desktop windows and mobile runners.
#[component]
pub fn App() -> impl IntoElement {
    let attempt = signal(0_u32);
    view().flex_1().bg(theme::PAPER).child(dynamic(move || {
        let _ = attempt.get();
        untracked(|| match state::State::load() {
            Ok(loaded) => mount(signal_with(loaded, SignalEquals::Never)),
            Err(error) => recovery::screen(error, attempt),
        })
    }))
}

fn mount(state: state::AppState) -> AnyElement {
    use state::Page;
    effect(move || {
        let appearance = state.with(|s| match s.snapshot.settings.theme.as_str() {
            "light" => nsh_core::platform::Appearance::Light,
            "dark" => nsh_core::platform::Appearance::Dark,
            _ => nsh_core::platform::Appearance::System,
        });
        set_color_scheme(appearance);
    });
    on_back_press(move || {
        let can_go = state.with(|s| s.detail.is_some() || s.page != Page::Today);
        if can_go {
            state.update(|s| {
                if s.detail.is_some() {
                    s.detail = None;
                    s.audio = None;
                } else {
                    s.navigate(Page::Today)
                }
            });
        }
        can_go
    });
    on_key("escape", move |_| {
        state.update(|s| {
            if s.detail.is_some() {
                s.detail = None;
                s.audio = None;
            } else {
                s.navigate(Page::Today)
            }
        })
    });
    on_key("enter", move |_| {
        state.update(|s| {
            let advance = s.page == Page::Learn
                && s.snapshot.progress.session.as_ref().is_some_and(|j| {
                    j.pending.is_some()
                        || j.steps
                            .get(j.index)
                            .is_some_and(|step| step.kind == atlas_domain::model::StepKind::Intro)
                });
            if advance {
                s.advance();
            }
        })
    });
    for key in ["1", "2", "3", "4"] {
        on_key(key, move |_| {
            let index = key.as_bytes()[0] as usize - b'1' as usize;
            let choice = state.with(|s| {
                if s.page == Page::Learn {
                    s.snapshot
                        .progress
                        .session
                        .as_ref()
                        .filter(|j| j.pending.is_none())
                        .and_then(|j| j.steps.get(j.index))
                        .and_then(|step| step.options.as_ref())
                        .and_then(|options| options.get(index))
                        .cloned()
                } else {
                    None
                }
            });
            if let Some(choice) = choice {
                state.update(|s| s.answer(&choice, false));
            }
        });
    }
    view()
        .flex_1()
        .bg(theme::PAPER)
        .child(shell::app(state))
        .child(audio::layers(state))
        .into_element()
}
