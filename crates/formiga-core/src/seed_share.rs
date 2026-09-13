use crate::{CreatureDesign, CreatureOrigin};
use sha2::{Digest, Sha256};

const FORMAT_VERSION: u8 = 1;
const PREFIX: &str = "FORMIGA";
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const PAYLOAD_BYTES: usize = 37;
const ENCODED_CHARACTERS: usize = 60;
const GROUPS: usize = 15;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SharedCreatureSeed {
    pub design: Option<CreatureDesign>,
    pub source_colony_seed: [u8; 32],
    pub source_generation: u8,
}

impl From<CreatureOrigin> for SharedCreatureSeed {
    fn from(origin: CreatureOrigin) -> Self {
        Self {
            design: origin.design,
            source_colony_seed: origin.source_colony_seed,
            source_generation: origin.source_generation,
        }
    }
}

impl From<SharedCreatureSeed> for CreatureOrigin {
    fn from(shared: SharedCreatureSeed) -> Self {
        Self {
            design: shared.design,
            source_colony_seed: shared.source_colony_seed,
            source_generation: shared.source_generation,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SeedCodeError {
    #[error("seed code must begin with FORMIGA")]
    Prefix,
    #[error("seed code must contain fifteen or twenty-three groups of four characters")]
    Format,
    #[error("seed code has an unsupported format version")]
    Version,
    #[error("seed code contains an unsupported source generation")]
    Generation,
    #[error("seed code contains an invalid Base32 character")]
    Character,
    #[error("seed code has an invalid length")]
    Length,
    #[error("seed code checksum does not match")]
    Checksum,
    #[error("seed code contains an invalid creature design")]
    Design,
}

pub fn encode_creature_seed(origin: CreatureOrigin) -> String {
    debug_assert!(origin.source_generation <= 3);
    let version = if origin.design.is_some() {
        2
    } else {
        FORMAT_VERSION
    };
    let mut payload = vec![
        0_u8;
        if origin.design.is_some() {
            57
        } else {
            PAYLOAD_BYTES
        }
    ];
    payload[0] = (version << 4) | origin.source_generation.min(3);
    payload[1..33].copy_from_slice(&origin.source_colony_seed);
    if let Some(design) = origin.design {
        payload[33..49].copy_from_slice(&design.to_bytes());
    }
    let checksum_start = payload.len() - 4;
    let digest = checksum(&payload[..checksum_start]);
    payload[checksum_start..].copy_from_slice(&digest);
    let encoded = encode_base32(&payload);
    debug_assert_eq!(
        encoded.len(),
        if version == 1 { ENCODED_CHARACTERS } else { 92 }
    );
    let grouped = encoded
        .as_bytes()
        .chunks(4)
        .map(|group| std::str::from_utf8(group).expect("Base32 is ASCII"))
        .collect::<Vec<_>>()
        .join("-");
    format!("{PREFIX}-{grouped}")
}

pub fn decode_creature_seed(code: &str) -> Result<SharedCreatureSeed, SeedCodeError> {
    if code.len() > 256 {
        return Err(SeedCodeError::Length);
    }
    let canonical = code.trim().to_ascii_uppercase();
    let Some(body) = canonical.strip_prefix("FORMIGA-") else {
        return Err(SeedCodeError::Prefix);
    };
    let groups: Vec<_> = body.split('-').collect();
    if ![GROUPS, 23].contains(&groups.len()) || groups.iter().any(|group| group.len() != 4) {
        return Err(SeedCodeError::Format);
    }
    let encoded = groups.concat();
    if ![ENCODED_CHARACTERS, 92].contains(&encoded.len()) {
        return Err(SeedCodeError::Length);
    }
    let payload = decode_base32(&encoded)?;
    let version = payload[0] >> 4;
    if ![FORMAT_VERSION, 2].contains(&version) {
        return Err(SeedCodeError::Version);
    }
    if payload.len() != if version == 1 { PAYLOAD_BYTES } else { 57 } {
        return Err(SeedCodeError::Length);
    }
    let generation = payload[0] & 0x0f;
    if generation > 3 {
        return Err(SeedCodeError::Generation);
    }
    let checksum_start = payload.len() - 4;
    if checksum(&payload[..checksum_start]) != payload[checksum_start..] {
        return Err(SeedCodeError::Checksum);
    }
    let design = if version == 2 {
        if payload[49..53] != [0; 4] {
            return Err(SeedCodeError::Design);
        }
        Some(CreatureDesign::from_bytes(&payload[33..49]).ok_or(SeedCodeError::Design)?)
    } else {
        None
    };
    let mut source_colony_seed = [0_u8; 32];
    source_colony_seed.copy_from_slice(&payload[1..33]);
    Ok(SharedCreatureSeed {
        design,
        source_colony_seed,
        source_generation: generation,
    })
}

pub fn derive_imported_colony_seed(shared: SharedCreatureSeed) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"formiga-imported-colony-v1");
    hash.update(shared.source_colony_seed);
    hash.update([shared.source_generation]);
    if let Some(design) = shared.design {
        hash.update(design.to_bytes());
    }
    let mut derived: [u8; 32] = hash.finalize().into();
    if derived == shared.source_colony_seed {
        derived[0] ^= 0x80;
    }
    derived
}

fn checksum(payload: &[u8]) -> [u8; 4] {
    let mut hash = Sha256::new();
    hash.update(b"formiga-shared-creature-v1");
    hash.update(payload);
    hash.finalize()[..4].try_into().unwrap()
}

fn encode_base32(bytes: &[u8]) -> String {
    let mut output = String::with_capacity((bytes.len() * 8).div_ceil(5));
    for character in 0..(bytes.len() * 8).div_ceil(5) {
        let mut value = 0_u8;
        for offset in 0..5 {
            let bit = character * 5 + offset;
            value <<= 1;
            if bit < bytes.len() * 8 {
                value |= (bytes[bit / 8] >> (7 - bit % 8)) & 1;
            }
        }
        output.push(ALPHABET[usize::from(value)] as char);
    }
    output
}

fn decode_base32(value: &str) -> Result<Vec<u8>, SeedCodeError> {
    let mut bits = Vec::with_capacity(value.len() * 5);
    for character in value.bytes() {
        let Some(index) = ALPHABET
            .iter()
            .position(|candidate| *candidate == character)
        else {
            return Err(SeedCodeError::Character);
        };
        for shift in (0..5).rev() {
            bits.push(((index >> shift) & 1) as u8);
        }
    }
    let payload_bytes = if value.len() == 92 { 57 } else { PAYLOAD_BYTES };
    let data_bits = payload_bytes * 8;
    if bits.len() < data_bits || bits[data_bits..].iter().any(|bit| *bit != 0) {
        return Err(SeedCodeError::Length);
    }
    let mut bytes = vec![0_u8; payload_bytes];
    for (index, bit) in bits.into_iter().take(data_bits).enumerate() {
        bytes[index / 8] |= bit << (7 - index % 8);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn design_codes_round_trip_and_validate_recipe_and_reserved_bytes() {
        for generation in 0..=3 {
            let shared = SharedCreatureSeed {
                source_colony_seed: [63; 32],
                source_generation: generation,
                design: Some(CreatureDesign::generated([63; 32], generation, None)),
            };
            let code = encode_creature_seed(shared.into());
            assert_eq!(code.split('-').skip(1).count(), 23);
            assert_eq!(decode_creature_seed(&code.to_lowercase()), Ok(shared));
            let encoded = code.strip_prefix("FORMIGA-").unwrap().replace('-', "");
            for offset in [33, 36, 49] {
                let mut payload = decode_base32(&encoded).unwrap();
                payload[offset] = 255;
                let digest = checksum(&payload[..53]);
                payload[53..].copy_from_slice(&digest);
                assert_eq!(
                    decode_creature_seed(&group_payload(&payload)),
                    Err(SeedCodeError::Design)
                );
            }
            let mut other = shared;
            other.design.as_mut().unwrap().coat[0] ^= 1;
            assert_ne!(
                derive_imported_colony_seed(shared),
                derive_imported_colony_seed(other)
            );
        }
    }

    #[test]
    fn all_four_generations_round_trip_case_insensitively() {
        for source_generation in 0_u8..=3 {
            let shared = SharedCreatureSeed {
                source_colony_seed: [source_generation.wrapping_mul(53).wrapping_add(7); 32],
                source_generation,
                design: None,
            };
            let code = encode_creature_seed(shared.into());
            assert!(code.starts_with("FORMIGA-"));
            assert_eq!(code.split('-').skip(1).count(), GROUPS);
            assert_eq!(decode_creature_seed(&code).unwrap(), shared);
            assert_eq!(decode_creature_seed(&code.to_lowercase()).unwrap(), shared);
        }
    }

    #[test]
    fn single_character_corruption_is_rejected() {
        let code = encode_creature_seed(
            SharedCreatureSeed {
                source_colony_seed: [91; 32],
                source_generation: 2,
                design: None,
            }
            .into(),
        );
        let mut bytes = code.into_bytes();
        let index = bytes
            .iter()
            .enumerate()
            .find(|(index, byte)| *index > 16 && **byte != b'-')
            .map(|(index, _)| index)
            .unwrap();
        bytes[index] = if bytes[index] == b'0' { b'1' } else { b'0' };
        assert_eq!(
            decode_creature_seed(std::str::from_utf8(&bytes).unwrap()),
            Err(SeedCodeError::Checksum)
        );
    }

    #[test]
    fn imported_lineage_is_distinct_and_deterministic() {
        let shared = SharedCreatureSeed {
            source_colony_seed: [44; 32],
            source_generation: 3,
            design: None,
        };
        let first = derive_imported_colony_seed(shared);
        let second = derive_imported_colony_seed(shared);
        assert_eq!(first, second);
        assert_ne!(first, shared.source_colony_seed);
    }

    #[test]
    fn every_seed_code_boundary_is_validated_before_import() {
        let valid = encode_creature_seed(
            SharedCreatureSeed {
                source_colony_seed: [123; 32],
                source_generation: 1,
                design: None,
            }
            .into(),
        );
        assert_eq!(
            decode_creature_seed(&valid.replacen("FORMIGA", "ANT", 1)),
            Err(SeedCodeError::Prefix)
        );
        assert_eq!(
            decode_creature_seed(&valid.replacen('-', "", 1)),
            Err(SeedCodeError::Prefix)
        );
        let mut malformed_groups = valid.clone();
        let separator = malformed_groups[8..].find('-').unwrap() + 8;
        malformed_groups.remove(separator);
        assert_eq!(
            decode_creature_seed(&malformed_groups),
            Err(SeedCodeError::Format)
        );
        let mut invalid_character = valid.clone().into_bytes();
        let character = invalid_character
            .iter()
            .enumerate()
            .find(|(index, byte)| *index > 12 && **byte != b'-')
            .map(|(index, _)| index)
            .unwrap();
        invalid_character[character] = b'I';
        assert_eq!(
            decode_creature_seed(std::str::from_utf8(&invalid_character).unwrap()),
            Err(SeedCodeError::Character)
        );

        let encoded = valid.strip_prefix("FORMIGA-").unwrap().replace('-', "");
        let mut payload = decode_base32(&encoded).unwrap();
        payload[0] = 3 << 4;
        let digest = checksum(&payload[..33]);
        payload[33..].copy_from_slice(&digest);
        assert_eq!(
            decode_creature_seed(&group_payload(&payload)),
            Err(SeedCodeError::Version)
        );
        payload[0] = (FORMAT_VERSION << 4) | 4;
        let digest = checksum(&payload[..33]);
        payload[33..].copy_from_slice(&digest);
        assert_eq!(
            decode_creature_seed(&group_payload(&payload)),
            Err(SeedCodeError::Generation)
        );

        let mut invalid_padding = valid.into_bytes();
        let last = invalid_padding.len() - 1;
        invalid_padding[last] = b'1';
        assert_eq!(
            decode_creature_seed(std::str::from_utf8(&invalid_padding).unwrap()),
            Err(SeedCodeError::Length)
        );
    }

    fn group_payload(payload: &[u8]) -> String {
        let encoded = encode_base32(payload);
        let grouped = encoded
            .as_bytes()
            .chunks(4)
            .map(|group| std::str::from_utf8(group).unwrap())
            .collect::<Vec<_>>()
            .join("-");
        format!("FORMIGA-{grouped}")
    }
}
