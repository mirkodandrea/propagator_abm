//! The score and the leaderboard (user, 2026-10-09).
//!
//! The score is made of the debrief's own numbers, so the screen and the
//! points never disagree: families the fire did not catch at home and homes
//! it did not hit, against the same fire with no orders ([`Game::without_orders`]),
//! plus the pauses the player did not need. Nothing here reads the
//! simulation: it only weighs two [`Outcome`]s.
//!
//! [`Game::without_orders`]: crate::Game::without_orders

use serde::{Deserialize, Serialize};

use crate::Outcome;

/// Points per family not caught at home, against no orders.
pub const PER_FAMILY: i64 = 100;
/// Points per home not hit, against no orders.
pub const PER_HOME: i64 = 10;
/// Points per pause left unused, only on top of a game that saved someone
/// or something: doing nothing must not score.
pub const PER_PAUSE: i64 = 50;
/// Places on the leaderboard.
pub const TOP: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Score {
    /// Families not caught at home thanks to the player (may be negative).
    pub families: i64,
    /// Homes not hit thanks to the player (may be negative).
    pub homes: i64,
    /// Pauses left unused.
    pub pauses: u32,
    pub total: i64,
}

/// The player's game against the same fire with no orders.
pub fn score(mine: &Outcome, without: &Outcome, pauses_left: u32) -> Score {
    let families = without.caught() as i64 - mine.caught() as i64;
    let homes = without.homes_hit() as i64 - mine.homes_hit() as i64;
    let saved = families * PER_FAMILY + homes * PER_HOME;
    let pauses = if saved > 0 { pauses_left } else { 0 };
    Score { families, homes, pauses, total: (saved + pauses as i64 * PER_PAUSE).max(0) }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// Three letters, arcade style: no personal data at a school event.
    pub initials: String,
    pub score: i64,
    /// The fire played, for the operator; one board for all of them.
    pub case: String,
}

/// The best [`TOP`] games, highest first; ties keep the earlier game above.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Board {
    pub entries: Vec<Entry>,
}

impl Board {
    /// Whether a score would enter the board.
    pub fn qualifies(&self, score: i64) -> bool {
        score > 0 && (self.entries.len() < TOP || self.entries.last().is_some_and(|e| score > e.score))
    }

    /// Add a game; returns its place (0 = first) if it stays on the board.
    pub fn insert(&mut self, e: Entry) -> Option<usize> {
        if !self.qualifies(e.score) {
            return None;
        }
        let at = self.entries.iter().position(|x| e.score > x.score).unwrap_or(self.entries.len());
        self.entries.insert(at, e);
        self.entries.truncate(TOP);
        Some(at)
    }

    /// Read a saved board; anything unreadable is an empty board, so a
    /// damaged file never stops the kiosk.
    pub fn from_json(s: &str) -> Board {
        serde_json::from_str(s).unwrap_or_default()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

/// Three letters from what was typed: upper case A-Z, padded with «A».
pub fn initials(letters: [u8; 3]) -> String {
    letters.iter().map(|&l| (b'A' + l % 26) as char).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DistrictOutcome;

    fn outcome(caught: usize, homes_hit: usize) -> Outcome {
        let d = DistrictOutcome { caught, homes_hit, ..Default::default() };
        Outcome { at_s: 0, districts: vec![d], hectares: 0.0, units_lost: 0 }
    }

    #[test]
    fn the_score_is_the_debriefs_numbers() {
        // the second game of the 2026-10-09 playtest: 2 / 24 against 16 / 73
        let s = score(&outcome(2, 24), &outcome(16, 73), 2);
        assert_eq!((s.families, s.homes, s.pauses), (14, 49, 2));
        assert_eq!(s.total, 14 * PER_FAMILY + 49 * PER_HOME + 2 * PER_PAUSE);
    }

    #[test]
    fn doing_nothing_does_not_score() {
        let s = score(&outcome(16, 73), &outcome(16, 73), 3);
        assert_eq!(s.total, 0);
        // worse than no orders: no negative points either
        assert_eq!(score(&outcome(20, 80), &outcome(16, 73), 3).total, 0);
    }

    #[test]
    fn the_board_keeps_the_best_ten() {
        let mut b = Board::default();
        for k in 1..=12 {
            b.insert(Entry { initials: "AAA".into(), score: k * 100, case: "c".into() });
        }
        assert_eq!(b.entries.len(), TOP);
        assert_eq!(b.entries[0].score, 1200);
        assert!(!b.qualifies(300));
        assert_eq!(b.insert(Entry { initials: "BBB".into(), score: 1200, case: "c".into() }), Some(1), "a tie goes below the earlier game");
        assert_eq!(Board::from_json(&b.to_json()), b);
        assert_eq!(Board::from_json("not json"), Board::default());
    }

    #[test]
    fn initials_are_three_letters() {
        assert_eq!(initials([0, 1, 25]), "ABZ");
    }
}
