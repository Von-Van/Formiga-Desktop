//! The Handler and their trios (version 3).
//!
//! A Handler is who the player chooses to be in Battle: a name, an optional club and a pennant.
//! A trio is three of the colony's companions the Handler has named as a team. Both are kept in
//! the colony's [`BattleRecord`] with everything else Battle earns, and both are identity, never
//! advantage: nothing here changes how a match is played.
//!
//! Battle says what the Handler card and the trios are ([`Registry`], in its result) and Desktop
//! keeps them by one rule, [`BattleRecord::register`]. A trio's record (its matches, wins, cup
//! wins and the companions it played with before) is only ever added to by a match, through
//! [`BattleRecord::apply`], so editing a trio can never change what it has done.
//!
//! A trio's members may change. Its record carries on under its id, and the companions it had
//! before are kept as earlier lineups, with when each lineup ended, so a past match is never
//! credited to someone who was not there. Since version 4 each match says which three played as
//! the trio ([`TrioPlayed::members`]), so a lineup that played and was changed within one session
//! is kept as an earlier lineup too ([`BattleRecord::register_session`]).
//!
//! Since version 5 the record also carries the Handler key ([`BattleRecord::handler_key`]): 128
//! random bits Desktop makes once for the colony, the first time it opens Battle, and keeps with
//! the record. It says nothing about the player, the colony or the computer; it only lets a
//! friend's Battle know the same Handler again. Battle reads it and never sends it back, so only
//! Desktop ever makes or changes one.

use crate::BattleRecord;
use crate::result::{CupWins, MatchResult, Mode};
use formiga_expansion_rulebook::is_sanitized;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use time::OffsetDateTime;

pub mod handler_limits {
    pub const MAX_HANDLER_CHARS: usize = 24;
    pub const MAX_CLUB_CHARS: usize = 32;
    pub const MAX_TRIO_CHARS: usize = 32;
    /// Trios kept, retired ones included.
    pub const MAX_TRIOS: usize = 12;
    /// Earlier lineups a trio keeps.
    pub const MAX_LINEUPS: usize = 8;
    /// Cups a trio keeps its wins toward.
    pub const MAX_TRIO_CUPS: usize = 8;
    pub const MAX_STYLE_CHARS: usize = 24;
    pub const TRIO_ID_CHARS: usize = 16;
    pub const HANDLER_KEY_CHARS: usize = 32;
}
use handler_limits::*;

/// A pennant, as four choices from Battle's own lists, by Battle's names for them. What each
/// looks like is Battle's; a name Battle no longer knows is drawn as its first choice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pennant {
    pub shape: String,
    pub field: String,
    pub symbol: String,
    pub trim: String,
}

/// Who the player is in Battle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandlerCard {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub club: Option<String>,
    pub pennant: Pennant,
}

/// Matches played, won and drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tally {
    pub played: u32,
    pub won: u32,
    pub drawn: u32,
}

impl Tally {
    pub fn is_empty(&self) -> bool {
        self.played == 0
    }

    pub fn lost(&self) -> u32 {
        self.played - self.won - self.drawn
    }

    fn add(&mut self, won: Option<bool>) {
        self.played = self.played.saturating_add(1);
        match won {
            Some(true) => self.won = self.won.saturating_add(1),
            None => self.drawn = self.drawn.saturating_add(1),
            Some(false) => {}
        }
    }

    fn holds(&self) -> bool {
        u64::from(self.won) + u64::from(self.drawn) <= u64::from(self.played)
    }
}

/// The three a trio had before its members last changed, and when that lineup ended.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lineup {
    pub members: Vec<u64>,
    #[serde(with = "time::serde::rfc3339")]
    pub until_utc: OffsetDateTime,
}

/// Three companions the Handler has named as a team, and what they have done as one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trio {
    /// 16 lowercase hex digits, chosen by Battle when the trio is named. Never changes.
    pub id: String,
    pub name: String,
    /// The three it has now, by creature id, smallest first.
    pub members: Vec<u64>,
    /// Its own pennant; without one it flies the Handler's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pennant: Option<Pennant>,
    /// Kept with its record, but no longer recognised when its three are picked.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub retired: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub formed_at_utc: OffsetDateTime,
    /// When it came to have the three it has now.
    #[serde(with = "time::serde::rfc3339")]
    pub members_since_utc: OffsetDateTime,
    #[serde(default, skip_serializing_if = "Tally::is_empty")]
    pub friends: Tally,
    #[serde(default, skip_serializing_if = "Tally::is_empty")]
    pub visitors: Tally,
    /// Wins toward each cup, as this trio.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cups: Vec<CupWins>,
    /// The three it had before, oldest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lineups: Vec<Lineup>,
}

/// A trio as Battle says it is: everything the Handler may change, and nothing it has earned.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrioCard {
    pub id: String,
    pub name: String,
    pub members: Vec<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pennant: Option<Pennant>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub retired: bool,
}

/// The Handler card and trios as they stand at the end of a session, for Desktop to keep. A trio
/// left out is kept as it was; nothing here can remove one.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Registry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handler: Option<HandlerCard>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trios: Vec<TrioCard>,
}

/// The trio a match was played as, and how it went: `won` is `None` for a draw.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrioPlayed {
    pub id: String,
    #[serde(default)]
    pub won: Option<bool>,
    /// The three who played as it, smallest id first. Since version 4; empty before, when the
    /// three it had at the end of the session are all that is known.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<u64>,
}

fn is_style(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= MAX_STYLE_CHARS
        && text
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_-".contains(&b))
}

/// A trio's id: 16 lowercase hex digits.
pub fn is_trio_id(id: &str) -> bool {
    id.len() == TRIO_ID_CHARS
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A Handler key: 32 lowercase hex digits.
pub fn is_handler_key(key: &str) -> bool {
    key.len() == HANDLER_KEY_CHARS
        && key
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Three different companions, smallest id first.
pub fn is_lineup(members: &[u64]) -> bool {
    members.len() == 3 && members.windows(2).all(|w| w[0] < w[1])
}

/// The same three, smallest id first, however they were picked.
pub fn lineup_of(members: &[u64]) -> Vec<u64> {
    let mut sorted = members.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    sorted
}

impl Pennant {
    pub fn holds(&self) -> bool {
        [&self.shape, &self.field, &self.symbol, &self.trim]
            .iter()
            .all(|part| is_style(part))
    }
}

impl HandlerCard {
    pub fn holds(&self) -> bool {
        is_sanitized(&self.name, MAX_HANDLER_CHARS)
            && self
                .club
                .as_deref()
                .is_none_or(|club| is_sanitized(club, MAX_CLUB_CHARS))
            && self.pennant.holds()
    }
}

impl TrioCard {
    pub fn holds(&self) -> bool {
        is_trio_id(&self.id)
            && is_sanitized(&self.name, MAX_TRIO_CHARS)
            && is_lineup(&self.members)
            && self.pennant.as_ref().is_none_or(Pennant::holds)
    }
}

impl Trio {
    pub fn holds(&self) -> bool {
        is_trio_id(&self.id)
            && is_sanitized(&self.name, MAX_TRIO_CHARS)
            && is_lineup(&self.members)
            && self.pennant.as_ref().is_none_or(Pennant::holds)
            && self.friends.holds()
            && self.visitors.holds()
            && self.cups.len() <= MAX_TRIO_CUPS
            && self.cups.iter().all(|c| crate::result::is_id(&c.cup))
            && self.lineups.len() <= MAX_LINEUPS
            && self.lineups.iter().all(|l| is_lineup(&l.members))
    }

    /// Everything it has played, against friends and the visitors together.
    pub fn total(&self) -> Tally {
        Tally {
            played: self.friends.played.saturating_add(self.visitors.played),
            won: self.friends.won.saturating_add(self.visitors.won),
            drawn: self.friends.drawn.saturating_add(self.visitors.drawn),
        }
    }

    /// The trio as Battle would say it is.
    pub fn card(&self) -> TrioCard {
        TrioCard {
            id: self.id.clone(),
            name: self.name.clone(),
            members: self.members.clone(),
            pennant: self.pennant.clone(),
            retired: self.retired,
        }
    }
}

impl Registry {
    pub fn is_empty(&self) -> bool {
        self.handler.is_none() && self.trios.is_empty()
    }

    pub fn holds(&self) -> bool {
        let ids: BTreeSet<&str> = self.trios.iter().map(|t| t.id.as_str()).collect();
        self.handler.as_ref().is_none_or(HandlerCard::holds)
            && self.trios.len() <= MAX_TRIOS
            && ids.len() == self.trios.len()
            && self.trios.iter().all(TrioCard::holds)
    }
}

impl TrioPlayed {
    pub fn holds(&self) -> bool {
        is_trio_id(&self.id) && (self.members.is_empty() || is_lineup(&self.members))
    }
}

impl BattleRecord {
    /// Keeps the Handler card and trios a session ends with, `now`. A trio already kept takes its
    /// new name, pennant and retirement; if its three changed, the three it had are kept as an
    /// earlier lineup ending `now`. A new trio is kept with an empty record, unless the record
    /// already has as many trios as it keeps, when the oldest retired one makes room for it, or,
    /// if none is retired, it is not kept. Trios already kept are updated before any new one is
    /// weighed, so a trio the registry retires makes room wherever it is listed. Keeping the same
    /// registry twice changes nothing more.
    pub fn register(&mut self, registry: &Registry, now: OffsetDateTime) {
        if let Some(card) = &registry.handler {
            self.handler = Some(card.clone());
        }
        let mut new = Vec::new();
        for card in &registry.trios {
            if let Some(trio) = self.trios.iter_mut().find(|t| t.id == card.id) {
                trio.name = card.name.clone();
                trio.pennant = card.pennant.clone();
                trio.retired = card.retired;
                if trio.members != card.members {
                    let earlier = std::mem::replace(&mut trio.members, card.members.clone());
                    trio.lineups.push(Lineup {
                        members: earlier,
                        until_utc: now,
                    });
                    let over = trio.lineups.len().saturating_sub(MAX_LINEUPS);
                    trio.lineups.drain(..over);
                    trio.members_since_utc = now;
                }
            } else {
                new.push(card);
            }
        }
        for card in new {
            if self.trios.len() >= MAX_TRIOS {
                match self.trios.iter().position(|t| t.retired) {
                    Some(oldest) => {
                        self.trios.remove(oldest);
                    }
                    None => continue,
                }
            }
            self.trios.push(Trio {
                id: card.id.clone(),
                name: card.name.clone(),
                members: card.members.clone(),
                pennant: card.pennant.clone(),
                retired: card.retired,
                formed_at_utc: now,
                members_since_utc: now,
                friends: Tally::default(),
                visitors: Tally::default(),
                cups: Vec::new(),
                lineups: Vec::new(),
            });
        }
    }

    /// The trio a match was played as gets the match on its record: against a friend or the
    /// visitors, and a cup win if the match won one. A trio the record does not have gets
    /// nothing.
    pub(crate) fn credit_trio(&mut self, mode: Mode, played: &TrioPlayed, cup: Option<&str>) {
        let Some(trio) = self.trios.iter_mut().find(|t| t.id == played.id) else {
            return;
        };
        match mode {
            Mode::Friend => trio.friends.add(played.won),
            Mode::Ai => trio.visitors.add(played.won),
        }
        if let Some(cup) = cup {
            match trio.cups.iter_mut().find(|c| c.cup == cup) {
                Some(c) => c.wins = c.wins.saturating_add(1),
                None => {
                    trio.cups.push(CupWins {
                        cup: cup.to_owned(),
                        wins: 1,
                    });
                    let over = trio.cups.len().saturating_sub(MAX_TRIO_CUPS);
                    trio.cups.drain(..over);
                }
            }
        }
    }

    /// The trio these three are, if they are one that is not retired.
    pub fn trio_of(&self, members: &[u64]) -> Option<&Trio> {
        let lineup = lineup_of(members);
        self.trios
            .iter()
            .find(|t| !t.retired && t.members == lineup)
    }

    /// Keeps the Handler card and trios a session ends with, as [`Self::register`] does, after
    /// first keeping each lineup a trio played with in `matches` (oldest first) that it no longer
    /// has: a trio that played as three and was then given another before the session ended keeps
    /// those three as an earlier lineup, ending `now` as every change kept from the session does.
    /// A match already applied, or one that does not say who played, changes nothing here, so
    /// keeping the same session twice changes nothing more.
    pub fn register_session(
        &mut self,
        registry: &Registry,
        matches: &[MatchResult],
        now: OffsetDateTime,
    ) {
        for m in matches {
            let Some(played) = &m.rewards.trio else {
                continue;
            };
            if self.applied.contains(&m.id) || !is_lineup(&played.members) {
                continue;
            }
            let Some(card) = registry.trios.iter().find(|c| c.id == played.id) else {
                continue;
            };
            let then = TrioCard {
                members: played.members.clone(),
                ..card.clone()
            };
            self.register(
                &Registry {
                    handler: None,
                    trios: vec![then],
                },
                now,
            );
        }
        self.register(registry, now);
    }

    pub(crate) fn identity_holds(&self) -> bool {
        let ids: BTreeSet<&str> = self.trios.iter().map(|t| t.id.as_str()).collect();
        self.handler.as_ref().is_none_or(HandlerCard::holds)
            && self.trios.len() <= MAX_TRIOS
            && ids.len() == self.trios.len()
            && self.trios.iter().all(Trio::holds)
            && self.handler_key.as_deref().is_none_or(is_handler_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample;
    use crate::{BattleResult, MatchResult, Rewards};
    use time::macros::datetime;

    const LATER: OffsetDateTime = datetime!(2026-10-20 12:00 UTC);

    fn played(id: &str, mode: Mode, won: Option<bool>, cup: Option<&str>) -> MatchResult {
        MatchResult {
            id: id.to_owned(),
            mode,
            rewards: Rewards {
                cup_win: cup.map(str::to_owned),
                history: (mode == Mode::Friend).then(|| sample::record().history[0].clone()),
                trio: Some(TrioPlayed {
                    id: sample::TRIO.to_owned(),
                    won,
                    members: Vec::new(),
                }),
                ..Rewards::default()
            },
        }
    }

    #[test]
    fn a_trio_keeps_friend_and_visitor_matches_apart_once_each() {
        let mut record = sample::record();
        assert!(record.apply(&played(
            &"a".repeat(32),
            Mode::Ai,
            Some(true),
            Some("cup.backyard")
        )));
        assert!(record.apply(&played(&"b".repeat(32), Mode::Ai, None, None)));
        assert!(record.apply(&played(&"c".repeat(32), Mode::Friend, Some(false), None)));
        assert!(!record.apply(&played(&"c".repeat(32), Mode::Friend, Some(true), None)));
        let trio = &record.trios[0];
        assert_eq!(
            trio.visitors,
            Tally {
                played: 2,
                won: 1,
                drawn: 1
            }
        );
        assert_eq!(
            trio.friends,
            Tally {
                played: 2,
                won: 1,
                drawn: 0
            }
        );
        assert_eq!(trio.friends.lost(), 1);
        assert_eq!(
            trio.cups,
            vec![CupWins {
                cup: "cup.backyard".to_owned(),
                wins: 1
            }]
        );
        assert_eq!(trio.total().played, 4);
        assert!(record.validate().is_ok());
    }

    #[test]
    fn a_match_for_a_trio_the_record_does_not_have_credits_nobody() {
        let mut record = sample::record();
        let before = record.trios.clone();
        let mut m = played(&"d".repeat(32), Mode::Ai, Some(true), None);
        m.rewards.trio.as_mut().unwrap().id = "ffffffffffffffff".to_owned();
        assert!(record.apply(&m));
        assert_eq!(record.trios, before);
    }

    #[test]
    fn swapping_a_member_keeps_the_record_and_remembers_the_three_before() {
        let mut record = sample::record();
        let before = record.trios[0].clone();
        let mut card = before.card();
        card.members = lineup_of(&[card.members[0], card.members[1], 999]);
        card.name = "The Thistle Three, again".to_owned();
        let registry = Registry {
            handler: None,
            trios: vec![card.clone()],
        };
        record.register(&registry, LATER);
        let trio = &record.trios[0];
        assert_eq!(trio.members, card.members);
        assert_eq!(trio.name, card.name);
        assert_eq!(trio.friends, before.friends, "its record carries on");
        assert_eq!(trio.formed_at_utc, before.formed_at_utc);
        assert_eq!(trio.members_since_utc, LATER);
        assert_eq!(
            trio.lineups,
            vec![Lineup {
                members: before.members.clone(),
                until_utc: LATER
            }]
        );
        let once = record.clone();
        record.register(&registry, LATER + time::Duration::days(1));
        assert_eq!(
            record, once,
            "keeping the same registry again changes nothing"
        );
        assert_eq!(
            record.handler,
            sample::record().handler,
            "a registry without a card keeps it"
        );
        assert!(record.validate().is_ok());
    }

    #[test]
    fn retired_trios_make_room_and_a_full_record_keeps_what_it_has() {
        let mut record = sample::record();
        let new = |n: u64, retired: bool| TrioCard {
            id: format!("{n:016x}"),
            name: format!("Trio {n}"),
            members: vec![n * 10, n * 10 + 1, n * 10 + 2],
            pennant: None,
            retired,
        };
        let cards: Vec<TrioCard> = (1..MAX_TRIOS as u64).map(|n| new(n, n == 3)).collect();
        record.register(
            &Registry {
                handler: None,
                trios: cards,
            },
            LATER,
        );
        assert_eq!(record.trios.len(), MAX_TRIOS);
        record.register(
            &Registry {
                handler: None,
                trios: vec![new(40, false)],
            },
            LATER,
        );
        assert_eq!(record.trios.len(), MAX_TRIOS);
        assert!(
            record.trios.iter().all(|t| t.id != format!("{:016x}", 3)),
            "the retired one made room"
        );
        record.register(
            &Registry {
                handler: None,
                trios: vec![new(41, false)],
            },
            LATER,
        );
        assert!(
            record.trios.iter().all(|t| t.id != format!("{:016x}", 41)),
            "no room, so not kept"
        );
        assert!(record.validate().is_ok());
    }

    /// A match against the visitors played by `members` as the sample's trio.
    fn played_by(id: &str, members: &[u64]) -> MatchResult {
        let mut m = played(id, Mode::Ai, Some(true), None);
        m.rewards.trio.as_mut().unwrap().members = lineup_of(members);
        m
    }

    #[test]
    fn a_lineup_that_played_and_changed_within_a_session_is_kept_as_an_earlier_one() {
        let mut record = sample::record();
        let before = record.trios[0].clone();
        let [a, b, c] = [before.members[0], before.members[1], before.members[2]];
        // Played as the three it came with, then as A, B and D, then as A, B and E, which is what
        // the session ended with.
        let matches = vec![
            played_by(&"1".repeat(32), &[a, b, c]),
            played_by(&"2".repeat(32), &[a, b, 900]),
            played_by(&"3".repeat(32), &[a, b, 901]),
        ];
        let card = TrioCard {
            members: lineup_of(&[a, b, 901]),
            ..before.card()
        };
        let registry = Registry {
            handler: None,
            trios: vec![card.clone()],
        };
        record.register_session(&registry, &matches, LATER);
        for m in &matches {
            assert!(record.apply(m));
        }
        let trio = &record.trios[0];
        assert_eq!(trio.members, card.members);
        assert_eq!(
            trio.lineups,
            vec![
                Lineup {
                    members: before.members.clone(),
                    until_utc: LATER
                },
                Lineup {
                    members: lineup_of(&[a, b, 900]),
                    until_utc: LATER
                },
            ],
            "the three it started with, then A, B and D, and nobody twice"
        );
        assert_eq!(trio.visitors.played, 3);
        assert!(record.validate().is_ok());
        let once = record.clone();
        record.register_session(&registry, &matches, LATER + time::Duration::days(1));
        assert_eq!(
            record, once,
            "keeping the same session again changes nothing"
        );
    }

    #[test]
    fn a_trio_named_played_and_changed_in_one_session_keeps_the_three_it_had() {
        let mut record = crate::BattleRecord::default();
        let id = "00aa00aa00aa00aa";
        let card = |members: &[u64]| TrioCard {
            id: id.to_owned(),
            name: "Odd Socks".to_owned(),
            members: lineup_of(members),
            pennant: None,
            retired: false,
        };
        let mut abc = played_by(&"4".repeat(32), &[1, 2, 3]);
        abc.rewards.trio.as_mut().unwrap().id = id.to_owned();
        // A lineup that changed back is kept each time it ended.
        let mut abd = played_by(&"5".repeat(32), &[1, 2, 4]);
        abd.rewards.trio.as_mut().unwrap().id = id.to_owned();
        let mut again = played_by(&"6".repeat(32), &[1, 2, 3]);
        again.rewards.trio.as_mut().unwrap().id = id.to_owned();
        let registry = Registry {
            handler: None,
            trios: vec![card(&[1, 2, 5])],
        };
        let matches = [abc, abd, again];
        record.register_session(&registry, &matches, LATER);
        let trio = &record.trios[0];
        assert_eq!(trio.members, vec![1, 2, 5]);
        let earlier: Vec<&[u64]> = trio.lineups.iter().map(|l| l.members.as_slice()).collect();
        assert_eq!(earlier, [&[1, 2, 3][..], &[1, 2, 4], &[1, 2, 3]]);
    }

    #[test]
    fn a_match_that_does_not_say_who_played_registers_as_before() {
        let mut with = sample::record();
        let mut without = sample::record();
        let before = with.trios[0].clone();
        let registry = Registry {
            handler: None,
            trios: vec![TrioCard {
                members: lineup_of(&[before.members[0], before.members[1], 999]),
                ..before.card()
            }],
        };
        let older = played(&"7".repeat(32), Mode::Ai, Some(true), None);
        with.register_session(&registry, std::slice::from_ref(&older), LATER);
        without.register(&registry, LATER);
        assert_eq!(with, without);
    }

    #[test]
    fn who_played_as_a_trio_is_three_different_companions_smallest_first() {
        let mut result = sample::result();
        assert!(result.validate().is_ok());
        for members in [vec![3, 2, 1], vec![1, 1, 2], vec![1, 2]] {
            result.matches[0].rewards.trio.as_mut().unwrap().members = members;
            assert!(result.validate().is_err());
        }
        result.matches[0].rewards.trio.as_mut().unwrap().members = Vec::new();
        assert!(result.validate().is_ok(), "a version 3 Battle says nothing");
    }

    #[test]
    fn a_trio_retired_in_the_same_registry_makes_room_wherever_it_is_listed() {
        let new = |n: u64, retired: bool| TrioCard {
            id: format!("{n:016x}"),
            name: format!("Trio {n}"),
            members: vec![n * 10, n * 10 + 1, n * 10 + 2],
            pennant: None,
            retired,
        };
        let mut full = sample::record();
        full.register(
            &Registry {
                handler: None,
                trios: (1..MAX_TRIOS as u64).map(|n| new(n, false)).collect(),
            },
            LATER,
        );
        assert_eq!(full.trios.len(), MAX_TRIOS);
        assert!(full.trios.iter().all(|t| !t.retired));
        for trios in [
            vec![new(40, false), new(3, true)],
            vec![new(3, true), new(40, false)],
        ] {
            let mut record = full.clone();
            record.register(
                &Registry {
                    handler: None,
                    trios,
                },
                LATER,
            );
            assert_eq!(record.trios.len(), MAX_TRIOS);
            assert!(record.trios.iter().any(|t| t.id == format!("{:016x}", 40)));
            assert!(record.trios.iter().all(|t| t.id != format!("{:016x}", 3)));
            assert!(record.validate().is_ok());
        }
    }

    #[test]
    fn a_trio_is_recognised_however_its_three_are_picked_unless_retired() {
        let mut record = sample::record();
        let mut picked = record.trios[0].members.clone();
        picked.reverse();
        assert_eq!(
            record.trio_of(&picked).map(|t| t.id.as_str()),
            Some(sample::TRIO)
        );
        assert!(record.trio_of(&picked[..2]).is_none());
        record.trios[0].retired = true;
        assert!(record.trio_of(&picked).is_none());
    }

    #[test]
    fn a_handler_card_or_trio_out_of_bounds_is_refused() {
        let ok = sample::result();
        assert!(ok.validate().is_ok());
        let refused = |change: &dyn Fn(&mut Registry)| {
            let mut result = ok.clone();
            change(result.registry.as_mut().unwrap());
            result.validate().is_err()
        };
        assert!(refused(
            &|r| r.handler.as_mut().unwrap().name = "x".repeat(25)
        ));
        assert!(refused(
            &|r| r.handler.as_mut().unwrap().name = "Jo\u{202E}".to_owned()
        ));
        assert!(refused(
            &|r| r.handler.as_mut().unwrap().club = Some(String::new())
        ));
        assert!(refused(
            &|r| r.handler.as_mut().unwrap().pennant.symbol = "Acorn".to_owned()
        ));
        assert!(refused(&|r| r.trios[0].members = vec![1, 1, 2]));
        assert!(refused(&|r| r.trios[0].members = vec![3, 2, 1]));
        assert!(refused(&|r| r.trios[0].id = "not hex at all!!".to_owned()));
        assert!(refused(&|r| r.trios[1].id = r.trios[0].id.clone()));
        assert!(refused(
            &|r| r.trios[0].name = " The Thistle Three".to_owned()
        ));
        let mut record = sample::record();
        record.trios[0].friends.won = 5;
        assert!(record.validate().is_err());
    }

    #[test]
    fn a_session_keeps_its_registry_then_credits_its_matches() {
        let result = sample::result();
        let bytes = result.encode().unwrap();
        let read = BattleResult::decode(&bytes).unwrap();
        assert_eq!(read, result);
        let mut record = crate::BattleRecord::default();
        record.register(read.registry.as_ref().unwrap(), LATER);
        for m in &read.matches {
            record.apply(m);
        }
        assert_eq!(record.trios.len(), 2);
        assert_eq!(record.trios[0].friends.played, 1);
        assert_eq!(record.trios[0].visitors.won, 1);
        assert_eq!(record.trios[0].cups[0].wins, 1);
        assert_eq!(record.handler.as_ref().unwrap().name, "Jo");
        assert!(record.validate().is_ok());
    }

    #[test]
    fn the_handler_key_is_desktops_and_nothing_battle_answers_changes_it() {
        let mut record = sample::record();
        assert!(record.validate().is_ok());
        let result = sample::result();
        record.register_session(result.registry.as_ref().unwrap(), &result.matches, LATER);
        for m in &result.matches {
            record.apply(m);
        }
        assert_eq!(record.handler_key.as_deref(), Some(sample::HANDLER_KEY));

        assert!(is_handler_key(sample::HANDLER_KEY));
        for odd in [
            "",
            "5F0C2A7E9B1D4C3A8E6F0B2D4A9C7E15",
            "5f0c2a7e9b1d4c3a",
            "5f0c2a7e9b1d4c3a8e6f0b2d4a9c7e15aa",
            "5f0c2a7e9b1d4c3a8e6f0b2d4a9c7eg5",
        ] {
            let mut odd_record = sample::record();
            odd_record.handler_key = Some(odd.to_owned());
            assert!(odd_record.validate().is_err(), "{odd:?} is not a key");
        }
        // A colony that has none yet is as before.
        let mut none = sample::record();
        none.handler_key = None;
        assert!(none.validate().is_ok());
        let snapshot = crate::sample::snapshot().with_record(none);
        assert!(
            !String::from_utf8(snapshot.encode().unwrap())
                .unwrap()
                .contains("handler_key")
        );
    }
}
