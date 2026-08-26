use std::sync::{Arc, Mutex, OnceLock};

use hashbrown::HashMap;
use shrimply_asset::{Asset, AssetSnapshot};
use shrimply_manim_ir::CompiledAnimation;

use crate::Settings;

type Cache = HashMap<ManimAnimationKey, Arc<CompiledAnimation>>;

static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();

#[derive(Clone, Eq, Hash, PartialEq)]
pub(super) struct ManimAnimationKey {
    source: AssetSnapshot,
    scene: String,
    width: u32,
    height: u32,
    fps: String,
    parameters: Vec<u8>,
}

pub(super) fn key(
    settings: &Settings,
    source: &AssetSnapshot,
) -> Result<ManimAnimationKey, String> {
    let mut parameters = settings.parameters.iter().collect::<Vec<_>>();
    parameters.sort_unstable_by_key(|(key, _)| *key);
    Ok(ManimAnimationKey {
        source: source.clone(),
        scene: settings.scene.clone(),
        width: settings.width,
        height: settings.height,
        fps: settings.fps.to_string(),
        parameters: rmp_serde::to_vec_named(&parameters)
            .map_err(|error| format!("encode Manim parameter cache key: {error}"))?,
    })
}

pub(super) fn get(key: &ManimAnimationKey) -> Result<Option<Arc<CompiledAnimation>>, String> {
    key.source.verify_current()?;
    let animation = {
        let mut cache = CACHE
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .expect("Manim IR cache poisoned");
        cache.retain(|cached, _| {
            cached.source.asset() != key.source.asset() || cached.source == key.source
        });
        cache.get(key).cloned()
    };
    let Some(animation) = animation else {
        return Ok(None);
    };
    tracing::info!(
        source = %key.source.path().display(),
        source_revision = key.source.revision(),
        scene = %key.scene,
        width = key.width,
        height = key.height,
        fps = %key.fps,
        "reusing compiled Manim animation",
    );
    key.source.verify_current()?;
    Ok(Some(animation))
}

pub(super) fn store(
    key: ManimAnimationKey,
    animation: Arc<CompiledAnimation>,
) -> Arc<CompiledAnimation> {
    if !key.source.is_current() {
        return animation;
    }
    let mut cache = CACHE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .expect("Manim IR cache poisoned");
    cache.retain(|cached, _| {
        cached.source.asset() != key.source.asset() || cached.source == key.source
    });
    if key.source.is_current() {
        cache.insert(key, animation.clone());
    }
    animation
}

pub(super) fn invalidate(source: &Asset) {
    if let Some(cache) = CACHE.get() {
        cache
            .lock()
            .expect("Manim IR cache poisoned")
            .retain(|key, _| key.source.asset() != source);
    }
}
