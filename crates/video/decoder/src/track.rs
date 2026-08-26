use std::hash::{Hash, Hasher};

use shrimply_asset::AssetSnapshot;
use shrimply_project::project::VideoItem;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum VideoPlane {
    Color,
    Alpha,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct VideoDecoderOwner {
    pub(crate) consumer: u64,
    pub(crate) sequence_path: Vec<Uuid>,
    pub(crate) track_id: Uuid,
    pub(crate) item_id: Uuid,
    pub(crate) plane: VideoPlane,
}

#[derive(Clone)]
pub(crate) struct VideoSource {
    pub(crate) asset: AssetSnapshot,
    pub(crate) media_track_id: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl VideoSource {
    pub(crate) fn for_item(item: &VideoItem) -> Result<Self, String> {
        Ok(Self {
            asset: item.file.snapshot()?,
            media_track_id: item.track_id,
            width: item.source_width.max(1),
            height: item.source_height.max(1),
        })
    }

    pub(crate) fn for_plane(item: &VideoItem, plane: VideoPlane) -> Result<Option<Self>, String> {
        match plane {
            VideoPlane::Color => Self::for_item(item).map(Some),
            VideoPlane::Alpha => {
                let Some(media_track_id) = item.alpha_mask_video else {
                    return Ok(None);
                };
                let mut alpha = item.clone();
                alpha.track_id = media_track_id;
                Self::for_item(&alpha).map(Some)
            }
        }
    }
}

impl PartialEq for VideoSource {
    fn eq(&self, other: &Self) -> bool {
        self.asset == other.asset && self.media_track_id == other.media_track_id
    }
}

impl Eq for VideoSource {}

impl Hash for VideoSource {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.asset.hash(state);
        self.media_track_id.hash(state);
    }
}
