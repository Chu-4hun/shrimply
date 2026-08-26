pub mod edit;
pub mod selection_state;

use shrimply_project::project::Time;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum TrackKind {
    Video,
    Caption,
    Audio,
}

impl TrackKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Video => "Video",
            Self::Caption => "Caption",
            Self::Audio => "Audio",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ItemKey {
    pub kind: TrackKind,
    pub track_index: usize,
    pub item_index: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct TrackKey {
    pub kind: TrackKind,
    pub track_index: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrackGap {
    pub track: TrackKey,
    pub start: Time,
    pub end: Time,
}
