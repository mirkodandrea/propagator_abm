//! Who owns the input this frame.
//!
//! The kiosk holds `keyboard` true for the whole session, so no single-key
//! shortcut can fire (finding 25). `pointer` is true while the cursor is over an
//! egui area, so a click on a button never reaches the map.

use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct UiFocus {
    pub pointer: bool,
    pub keyboard: bool,
}

impl UiFocus {
    /// True when the map should ignore the keyboard.
    pub fn typing(&self) -> bool {
        self.keyboard
    }
}
