use std::sync::{LazyLock, Mutex};

use hashbrown::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Status {
    Running {
        message: String,
        completed_frames: u64,
        total_frames: u64,
        prompt_signature: u64,
        server_url: String,
    },
    Complete {
        prompt_signature: u64,
    },
    Cancelling,
    Cancelled,
    Failed(String),
}

static STATUSES: LazyLock<Mutex<HashMap<Uuid, (u64, Status)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static CANCELLATIONS: LazyLock<
    Mutex<HashMap<Uuid, (u64, shrimply_server_client::CancellationToken)>>,
> = LazyLock::new(|| Mutex::new(HashMap::new()));
static CLAIMS: LazyLock<Mutex<HashSet<(Uuid, u64)>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

pub struct Claim {
    modifier_id: Uuid,
    generation: u64,
}

impl Drop for Claim {
    fn drop(&mut self) {
        CLAIMS
            .lock()
            .expect("SAM2 analysis claim lock is poisoned")
            .remove(&(self.modifier_id, self.generation));
    }
}

pub fn claim(modifier_id: Uuid, generation: u64) -> Option<Claim> {
    let statuses = STATUSES
        .lock()
        .expect("SAM2 analysis status lock is poisoned");
    if !statuses
        .get(&modifier_id)
        .is_some_and(|(stored_generation, status)| {
            *stored_generation == generation && matches!(status, Status::Running { .. })
        })
    {
        return None;
    }
    CLAIMS
        .lock()
        .expect("SAM2 analysis claim lock is poisoned")
        .insert((modifier_id, generation))
        .then_some(Claim {
            modifier_id,
            generation,
        })
}

pub fn start(modifier_id: Uuid, generation: u64, status: Status) {
    STATUSES
        .lock()
        .expect("SAM2 analysis status lock is poisoned")
        .insert(modifier_id, (generation, status));
}

pub fn update(modifier_id: Uuid, generation: u64, status: Status) -> bool {
    let mut statuses = STATUSES
        .lock()
        .expect("SAM2 analysis status lock is poisoned");
    let Some((stored_generation, stored_status)) = statuses.get_mut(&modifier_id) else {
        return false;
    };
    if *stored_generation != generation
        || matches!(stored_status, Status::Cancelling | Status::Cancelled)
    {
        return false;
    }
    *stored_status = status;
    true
}

pub fn is_current(modifier_id: Uuid, generation: u64) -> bool {
    STATUSES
        .lock()
        .expect("SAM2 analysis status lock is poisoned")
        .get(&modifier_id)
        .is_some_and(|(stored_generation, status)| {
            *stored_generation == generation && matches!(status, Status::Running { .. })
        })
}

pub fn cancel(modifier_id: Uuid, generation: u64) -> bool {
    let mut statuses = STATUSES
        .lock()
        .expect("SAM2 analysis status lock is poisoned");
    let Some((stored_generation, status)) = statuses.get_mut(&modifier_id) else {
        return false;
    };
    if *stored_generation != generation || !matches!(status, Status::Running { .. }) {
        return false;
    }
    let cancellation = CANCELLATIONS
        .lock()
        .expect("SAM2 cancellation lock is poisoned")
        .remove(&modifier_id)
        .filter(|(stored_generation, _)| *stored_generation == generation);
    *status = if cancellation.is_some() {
        Status::Cancelling
    } else {
        Status::Cancelled
    };
    if let Some((_, cancellation)) = cancellation {
        cancellation.cancel();
    }
    true
}

pub fn set_cancellation(
    modifier_id: Uuid,
    generation: u64,
    cancellation: shrimply_server_client::CancellationToken,
) -> bool {
    let statuses = STATUSES
        .lock()
        .expect("SAM2 analysis status lock is poisoned");
    let current = statuses
        .get(&modifier_id)
        .is_some_and(|(stored_generation, status)| {
            *stored_generation == generation && matches!(status, Status::Running { .. })
        });
    if !current {
        drop(statuses);
        cancellation.cancel();
        return false;
    }
    CANCELLATIONS
        .lock()
        .expect("SAM2 cancellation lock is poisoned")
        .insert(modifier_id, (generation, cancellation));
    true
}

pub fn clear_cancellation(modifier_id: Uuid, generation: u64) {
    let mut cancellations = CANCELLATIONS
        .lock()
        .expect("SAM2 cancellation lock is poisoned");
    if cancellations
        .get(&modifier_id)
        .is_some_and(|(stored_generation, _)| *stored_generation == generation)
    {
        cancellations.remove(&modifier_id);
    }
    drop(cancellations);
    let mut statuses = STATUSES
        .lock()
        .expect("SAM2 analysis status lock is poisoned");
    if let Some((_, status)) = statuses
        .get_mut(&modifier_id)
        .filter(|(stored_generation, _)| *stored_generation == generation)
        && matches!(status, Status::Cancelling)
    {
        *status = Status::Cancelled;
    }
}

pub fn get(modifier_id: Uuid, generation: u64) -> Option<Status> {
    STATUSES
        .lock()
        .expect("SAM2 analysis status lock is poisoned")
        .get(&modifier_id)
        .filter(|(stored_generation, _)| *stored_generation == generation)
        .map(|(_, status)| status.clone())
}

pub fn server_url(modifier_id: Uuid, generation: u64) -> Option<String> {
    match get(modifier_id, generation)? {
        Status::Running { server_url, .. } => Some(server_url),
        _ => None,
    }
}
