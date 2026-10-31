//! Native screens; every page uses live domain state and installed media.

mod atlas;
mod galaxy;
mod learn;
mod me;
mod practice;
mod today;
mod word;

pub(crate) use atlas::atlas;
pub(crate) use galaxy::galaxy;
pub(crate) use learn::learn;
pub(crate) use me::{about, me, onboarding};
pub(crate) use practice::practice;
pub(crate) use today::today;
pub(crate) use word::word_detail;
