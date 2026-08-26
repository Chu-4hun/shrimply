use hashbrown::HashMap;
use std::sync::{Mutex, OnceLock};

use uuid::Uuid;

static ERRORS: OnceLock<Mutex<HashMap<Uuid, (u64, String)>>> = OnceLock::new();
type ParameterStatus = (u64, String, Vec<shrimply_project::project::ManimParameter>);
static PARAMETERS: OnceLock<Mutex<HashMap<Uuid, ParameterStatus>>> = OnceLock::new();

pub fn set_parameters(
    item_id: Uuid,
    source_revision: u64,
    scene: String,
    parameters: Vec<shrimply_project::project::ManimParameter>,
) -> bool {
    let mut values = PARAMETERS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .expect("Manim parameters lock is poisoned");
    let next = (source_revision, scene, parameters);
    if values.get(&item_id) == Some(&next) {
        false
    } else {
        values.insert(item_id, next);
        true
    }
}

pub fn parameters(
    item_id: Uuid,
    source_revision: u64,
    scene: &str,
) -> Option<Vec<shrimply_project::project::ManimParameter>> {
    PARAMETERS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .expect("Manim parameters lock is poisoned")
        .get(&item_id)
        .filter(|(revision, stored_scene, _)| *revision == source_revision && stored_scene == scene)
        .map(|(_, _, parameters)| parameters.clone())
}

pub fn set_error(item_id: Uuid, source_revision: u64, error: Option<String>) -> bool {
    let mut errors = ERRORS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .expect("Manim status lock is poisoned");
    match error {
        Some(error) => {
            if errors.get(&item_id) == Some(&(source_revision, error.clone())) {
                false
            } else {
                errors.insert(item_id, (source_revision, error));
                true
            }
        }
        None if errors
            .get(&item_id)
            .is_some_and(|(revision, _)| *revision == source_revision) =>
        {
            errors.remove(&item_id);
            true
        }
        None => false,
    }
}

pub fn error(item_id: Uuid, source_revision: u64) -> Option<String> {
    ERRORS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .expect("Manim status lock is poisoned")
        .get(&item_id)
        .filter(|(revision, _)| *revision == source_revision)
        .map(|(_, error)| error.clone())
}
