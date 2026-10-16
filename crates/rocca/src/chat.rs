//! The group chat of the operation: short, colloquial lines that tell what is
//! happening, in the voice of the people involved. Only what matters on the
//! ground; the game writes them (`Game::say`), the kiosk draws them.

/// Who is writing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Voice {
    /// The team with this index in `Game::teams`.
    Team(usize),
    Mayor,
    Expert,
}

/// What a line is about: one line per voice and kind within `REPEAT_S`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Order,
    Arrived,
    Withdraw,
    Lost,
    EvacStart,
    EvacRoute,
    EvacStuck,
    EvacDone,
    EvacCancel,
    Wind,
}

/// A voice does not repeat a kind of line within this many simulated seconds.
pub const REPEAT_S: i64 = 600;

#[derive(Debug, Clone, PartialEq)]
pub struct ChatMsg {
    pub at_s: i64,
    pub who: Voice,
    pub kind: Kind,
    pub text: String,
}
