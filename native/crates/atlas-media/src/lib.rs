//! Stable offline asset identifiers used equally by development and full installation packages.

/// Available prerecorded audio variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioKind {
    /// British word recording.
    Uk,
    /// American word recording.
    Us,
    /// First example sentence.
    Example,
    /// Optional IELTS example sentence.
    ExampleTwo,
}
impl AudioKind {
    /// Resource subdirectory and its bit flag in the vocabulary index.
    pub fn resource(self) -> (&'static str, u8) {
        match self {
            Self::Uk => ("uk", 2),
            Self::Us => ("us", 4),
            Self::Example => ("ex", 8),
            Self::ExampleTwo => ("ex2", 16),
        }
    }
}

/// A resource identifier is invalid or was not included in the canonical media manifest.
#[derive(Debug, thiserror::Error)]
#[error("无效的资源标识：{0}")]
pub struct Error(String);

fn valid_id(id: &str) -> Result<(), Error> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == b'-' || ch == b'_')
    {
        Err(Error(id.into()))
    } else {
        Ok(())
    }
}

/// URI of an installed illustration, with no remote fallback.
pub fn illustration(id: &str, thumbnail: bool) -> Result<String, Error> {
    valid_id(id)?;
    Ok(format!(
        "asset:/atlas/{}/{id}.webp",
        if thumbnail { "thumb" } else { "img" }
    ))
}
/// URI of a prerecorded installed clip, returning none if the index says it is unavailable.
pub fn audio(id: &str, kind: AudioKind, media: u8) -> Result<Option<String>, Error> {
    valid_id(id)?;
    let (folder, flag) = kind.resource();
    Ok((media & flag != 0).then(|| format!("asset:/atlas/audio/{folder}/{id}.mp3")))
}
/// Installed brand artwork.
pub fn brand(name: &str) -> Result<String, Error> {
    valid_id(name)?;
    Ok(format!("asset:/brand/{name}.webp"))
}
/// Installed interface sound or ambience clip; extensions are fixed by the resource manifest.
pub fn sound(name: &str) -> Result<String, Error> {
    valid_id(name)?;
    Ok(format!("asset:/sfx/{name}.mp3"))
}
