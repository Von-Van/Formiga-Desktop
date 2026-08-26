use getrandom::fill;
use rand::SeedableRng;
use rand_chacha::ChaCha12Rng;
use sha2::{Digest, Sha256};

pub fn new_colony_seed() -> Result<[u8; 32], getrandom::Error> {
    let mut seed = [0_u8; 32];
    fill(&mut seed)?;
    Ok(seed)
}

#[derive(Clone)]
pub struct SeedStream {
    root: [u8; 32],
}

impl SeedStream {
    pub fn new(root: [u8; 32]) -> Self {
        Self { root }
    }

    pub fn bytes(&self, label: &str, index: u64) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(self.root);
        hash.update((label.len() as u64).to_le_bytes());
        hash.update(label.as_bytes());
        hash.update(index.to_le_bytes());
        hash.finalize().into()
    }

    pub fn rng(&self, label: &str, index: u64) -> ChaCha12Rng {
        ChaCha12Rng::from_seed(self.bytes(label, index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    #[test]
    fn named_streams_are_stable_and_independent() {
        let streams = SeedStream::new([7; 32]);
        let a = streams.rng("appearance", 0).random::<u64>();
        let b = streams.rng("appearance", 0).random::<u64>();
        let c = streams.rng("personality", 0).random::<u64>();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
