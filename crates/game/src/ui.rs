//! Who owns the pointer this frame: true while the cursor is over an egui
//! panel, so a click on a button never reaches the map.

use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct UiFocus {
    pub pointer: bool,
}
