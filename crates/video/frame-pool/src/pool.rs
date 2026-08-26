use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use hashbrown::{HashMap, HashSet};
use shrimply_visual_frame::{Device, VisualFrame};

use crate::ImageKey;

struct Entry {
    frame: VisualFrame,
    last_access: u64,
    protected: bool,
}

#[derive(Default)]
struct PoolState {
    frames: HashMap<ImageKey, Entry>,
    loading: HashSet<ImageKey>,
    failed: HashMap<ImageKey, String>,
    bytes: u64,
    access: u64,
}

pub struct ImagePool {
    state: Mutex<PoolState>,
    maximum_bytes: AtomicU64,
}

impl ImagePool {
    pub(crate) fn new(maximum_bytes: u64) -> Self {
        Self {
            state: Mutex::new(PoolState::default()),
            maximum_bytes: AtomicU64::new(maximum_bytes),
        }
    }

    pub fn get(&self, key: &ImageKey) -> Result<Option<VisualFrame>, String> {
        let mut state = self.state.lock().expect("image pool mutex poisoned");
        if let Some(error) = state.failed.get(key) {
            return Err(error.clone());
        }
        let access = next_access(&mut state);
        let Some(entry) = state.frames.get_mut(key) else {
            return Ok(None);
        };
        entry.last_access = access;
        entry.protected = true;
        Ok(Some(entry.frame.clone()))
    }

    pub fn contains(&self, key: &ImageKey) -> bool {
        let state = self.state.lock().expect("image pool mutex poisoned");
        state.frames.contains_key(key)
            || state.loading.contains(key)
            || state.failed.contains_key(key)
    }

    pub fn begin_load(&self, key: ImageKey) -> bool {
        let mut state = self.state.lock().expect("image pool mutex poisoned");
        if state.frames.contains_key(&key)
            || state.loading.contains(&key)
            || state.failed.contains_key(&key)
        {
            return false;
        }
        state.loading.insert(key);
        update_counters(&state);
        true
    }

    pub fn finish_load(
        &self,
        key: ImageKey,
        result: Result<VisualFrame, String>,
    ) -> Result<(), String> {
        let result = result.and_then(to_cpu);
        let mut state = self.state.lock().expect("image pool mutex poisoned");
        state.loading.remove(&key);
        match result {
            Ok(frame) => insert_frame(
                &mut state,
                key,
                frame,
                false,
                self.maximum_bytes.load(Ordering::Acquire),
            ),
            Err(error) => {
                state.failed.insert(key, error);
            }
        }
        update_counters(&state);
        Ok(())
    }

    pub fn insert(&self, key: ImageKey, frame: VisualFrame) -> Result<(), String> {
        let frame = to_cpu(frame)?;
        let mut state = self.state.lock().expect("image pool mutex poisoned");
        state.loading.remove(&key);
        state.failed.remove(&key);
        insert_frame(
            &mut state,
            key,
            frame,
            false,
            self.maximum_bytes.load(Ordering::Acquire),
        );
        update_counters(&state);
        Ok(())
    }

    pub(crate) fn set_maximum_bytes(&self, maximum_bytes: u64) {
        self.maximum_bytes.store(maximum_bytes, Ordering::Release);
        let mut state = self.state.lock().expect("image pool mutex poisoned");
        evict_to_fit(&mut state, 0, maximum_bytes);
        update_counters(&state);
    }

    pub fn clear(&self) {
        let mut state = self.state.lock().expect("image pool mutex poisoned");
        *state = PoolState::default();
        update_counters(&state);
    }
}

fn to_cpu(frame: VisualFrame) -> Result<VisualFrame, String> {
    if frame.device() == Device::Cpu {
        Ok(frame)
    } else {
        frame.copy_to(Device::Cpu)
    }
}

fn insert_frame(
    state: &mut PoolState,
    key: ImageKey,
    frame: VisualFrame,
    mut protected: bool,
    maximum_bytes: u64,
) {
    if frame.bytes() > maximum_bytes {
        return;
    }
    if let Some(previous) = state.frames.remove(&key) {
        protected |= previous.protected;
        state.bytes = state
            .bytes
            .checked_sub(previous.frame.bytes())
            .expect("frame cache byte accounting underflowed while replacing a frame");
    }
    evict_to_fit(state, frame.bytes(), maximum_bytes);
    state.bytes = state.bytes.saturating_add(frame.bytes());
    let last_access = next_access(state);
    state.frames.insert(
        key,
        Entry {
            frame,
            last_access,
            protected,
        },
    );
}

fn evict_to_fit(state: &mut PoolState, additional_bytes: u64, maximum_bytes: u64) {
    while state.bytes.saturating_add(additional_bytes) > maximum_bytes {
        let Some(key) = state
            .frames
            .iter()
            .min_by_key(|(_, entry)| (entry.protected, entry.last_access))
            .map(|(key, _)| key.clone())
        else {
            break;
        };
        let entry = state
            .frames
            .remove(&key)
            .expect("frame disappeared before eviction");
        state.bytes = state
            .bytes
            .checked_sub(entry.frame.bytes())
            .expect("frame cache byte accounting underflowed while evicting a frame");
    }
}

fn next_access(state: &mut PoolState) -> u64 {
    state.access = state
        .access
        .checked_add(1)
        .expect("frame cache access counter overflowed");
    state.access
}

fn update_counters(state: &PoolState) {
    shrimply_benchmarking::set_counter("Image pool / CPU bytes retained", state.bytes);
    shrimply_benchmarking::set_counter(
        "Image pool / CPU images retained",
        state.frames.len() as u64,
    );
    shrimply_benchmarking::set_counter("Image pool / Loads pending", state.loading.len() as u64);
    shrimply_benchmarking::set_counter("Image pool / Failed images", state.failed.len() as u64);
}
