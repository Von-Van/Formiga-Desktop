//! The one answer every companion app gives in the same words: why it could not take a visit.

use serde::{Deserialize, Serialize};

/// Why a companion app could not take a visit, as its acknowledgement says.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AckRefusal {
    /// What Desktop wrote needs a newer reader than this app has; `reads` is the newest version of
    /// its contract it has.
    UnsupportedVersion { reads: u32 },
    /// What Desktop wrote did not check out.
    Invalid,
    /// The app is already taking a visit.
    Busy,
    /// Anything a newer app says that this build does not know.
    #[serde(other)]
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_refusal_is_written_as_hill_and_home_have_always_written_it() {
        for (refusal, written) in [
            (
                AckRefusal::UnsupportedVersion { reads: 6 },
                r#"{"kind":"unsupported_version","reads":6}"#,
            ),
            (AckRefusal::Invalid, r#"{"kind":"invalid"}"#),
            (AckRefusal::Busy, r#"{"kind":"busy"}"#),
            (AckRefusal::Other, r#"{"kind":"other"}"#),
        ] {
            assert_eq!(serde_json::to_string(&refusal).unwrap(), written);
            assert_eq!(
                serde_json::from_str::<AckRefusal>(written).unwrap(),
                refusal
            );
        }
        assert_eq!(
            serde_json::from_str::<AckRefusal>(r#"{"kind":"gone_fishing"}"#).unwrap(),
            AckRefusal::Other,
            "a reason from a newer app reads as other"
        );
    }
}
