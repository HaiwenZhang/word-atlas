//! Three native media channels: pronunciation, short interface cues and looping ambience.
//!
//! NSH's media element is backed by the platform player and can play local MP3 audio without a picture.

use nsh_core::{AudioSession, prelude::*};

use crate::state::AppState;

pub(crate) fn layers(state: AppState) -> AnyElement {
    let voice = computed(move || {
        state.with(|s| {
            s.audio
                .clone()
                .map(|(uri, rate)| (uri, rate, s.audio_sequence))
        })
    });
    let cue = computed(move || {
        state.with(|s| {
            s.snapshot
                .settings
                .sfx
                .then(|| s.cue.clone().map(|(uri, gain)| (uri, gain, s.cue_sequence)))
                .flatten()
        })
    });
    let ambience = computed(move || {
        state.with(|s| {
            let settings = &s.snapshot.settings;
            (settings.onboarded && settings.ambience != "off")
                .then(|| format!("asset:/sfx/amb-{}.mp3", settings.ambience))
        })
    });
    view().absolute().left_0().bottom_0().size(px(1.0)).opacity(0.0)
        .child(dynamic(move||voice.get().map(|(uri,rate,sequence)| {
            let path=uri.clone();let first_progress=stored(false);
            video(uri).key(format!("voice-{sequence}")).paused(false).controls(false).rate(rate).size(px(1.0))
                .audio_session(||AudioSession::MixWithOthers)
                .on_load(move|info:&VideoInfo|log::debug!(target:"word_atlas_audio","decoded {path}, duration {}s",info.duration))
                .on_progress(move|progress:&VideoProgress| {
                    if progress.current_time>0.0 && !first_progress.replace(true) {log::debug!(target:"word_atlas_audio","native pronunciation playback advanced to {}s",progress.current_time);}
                })
                .on_end(move||state.update(|s|{if s.audio_sequence==sequence {s.audio=None;}}))
                .on_error(move|error:&str|state.update(|s|{if s.audio_sequence==sequence {s.error=Some(format!("录音播放失败：{error}"));s.audio=None;}}))
        })))
        .child(dynamic(move||cue.get().map(|(uri,gain,sequence)|video(uri).key(format!("cue-{sequence}")).paused(false).controls(false).volume(gain).size(px(1.0)).audio_session(||AudioSession::MixWithOthers)
            .on_end(move||state.update(|s|{if s.cue_sequence==sequence {s.cue=None;}}))
            .on_error(move|error:&str|log::warn!("interface sound: {error}")))))
        .child(dynamic(move||ambience.get().map(|uri|video(uri).paused(false).controls(false).repeat(true).size(px(1.0)).audio_session(||AudioSession::MixWithOthers)
            .volume(move||state.with(|s|s.snapshot.settings.ambience_volume as f32*if s.audio.is_some(){0.315}else{0.9}))
            .on_error(move|error:&str|state.update(|s|s.message=Some(format!("环境声暂时无法播放：{error}")))))))
        .into_element()
}
