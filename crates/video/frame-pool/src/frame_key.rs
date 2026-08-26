use std::hash::{Hash, Hasher};
use std::path::PathBuf;

#[derive(Clone, Eq, PartialEq)]
pub struct ImageKey {
    source: PathBuf,
    discriminator: Vec<u8>,
}

impl ImageKey {
    pub fn new(source: PathBuf, discriminator: Vec<u8>) -> Self {
        Self {
            source,
            discriminator,
        }
    }
}

impl Hash for ImageKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.source.hash(state);
        self.discriminator.hash(state);
    }
}
