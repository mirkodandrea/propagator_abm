//! Repairs desktop text-edit shortcuts before egui begins its frame.
//!
//! `bevy_egui` 0.28 derives its modifier snapshot from logical modifier-key
//! events. On macOS those can disagree with Bevy's physical key state, which
//! leaves egui seeing `A` while Cmd is apparently up. The game already uses
//! `ButtonInput<KeyCode>` as the authoritative shortcut state, so text edits
//! should use the same source.

use bevy::prelude::*;
use bevy_egui::{egui, EguiClipboard, EguiInput, EguiSet};

pub struct NativeTextInputPlugin;

impl Plugin for NativeTextInputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PreUpdate,
            normalize_text_edit_input
                .after(EguiSet::ProcessInput)
                .before(EguiSet::BeginFrame),
        );
    }
}

fn normalize_text_edit_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut inputs: Query<&mut EguiInput>,
    mut clipboard: ResMut<EguiClipboard>,
) {
    let modifiers = modifiers_from_keys(&keys);

    for mut input in &mut inputs {
        let missing = normalize_events(&mut input, modifiers);
        for event in missing {
            match event {
                MissingClipboardEvent::Copy => input.events.push(egui::Event::Copy),
                MissingClipboardEvent::Cut => input.events.push(egui::Event::Cut),
                MissingClipboardEvent::Paste => {
                    if let Some(text) = clipboard.get_contents() {
                        input.events.push(egui::Event::Paste(text));
                    }
                }
            }
        }
    }
}

fn modifiers_from_keys(keys: &ButtonInput<KeyCode>) -> egui::Modifiers {
    let alt = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let super_key = keys.pressed(KeyCode::SuperLeft) || keys.pressed(KeyCode::SuperRight);
    let mac_cmd = cfg!(target_os = "macos") && super_key;

    egui::Modifiers {
        alt,
        ctrl,
        shift,
        mac_cmd,
        command: if cfg!(target_os = "macos") {
            super_key
        } else {
            ctrl
        },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MissingClipboardEvent {
    Copy,
    Cut,
    Paste,
}

/// Normalize every key event, and report clipboard events that bevy_egui did
/// not synthesize because its own modifier snapshot missed Cmd/Ctrl.
fn normalize_events(
    input: &mut egui::RawInput,
    modifiers: egui::Modifiers,
) -> Vec<MissingClipboardEvent> {
    input.modifiers = modifiers;
    let mut missing = Vec::new();
    let mut normalized = Vec::with_capacity(input.events.len());

    for mut event in std::mem::take(&mut input.events) {
        let egui::Event::Key {
            key,
            pressed,
            modifiers: event_modifiers,
            ..
        } = &mut event
        else {
            normalized.push(event);
            continue;
        };

        // If bevy_egui already saw the command modifier, its input system also
        // emitted the corresponding clipboard event. Remember that before
        // replacing the stale snapshot so Cut cannot accidentally run twice.
        let command_was_seen = event_modifiers.command;
        if modifiers.command && !command_was_seen {
            *event_modifiers = modifiers;

            // process_input_system emits text before the key event. If it
            // missed Cmd, Cmd-A arrives as `Text("a"), Key(A)`: repairing only
            // the latter would mutate the field before selecting it.
            if *pressed && preceding_text_matches(normalized.last(), *key) {
                normalized.pop();
            }
        }

        if *pressed && modifiers.command && !command_was_seen {
            match key {
                egui::Key::C => missing.push(MissingClipboardEvent::Copy),
                egui::Key::X => missing.push(MissingClipboardEvent::Cut),
                egui::Key::V => missing.push(MissingClipboardEvent::Paste),
                _ => {}
            }
        }

        normalized.push(event);
    }

    input.events = normalized;
    missing
}

fn preceding_text_matches(event: Option<&egui::Event>, key: egui::Key) -> bool {
    let Some(egui::Event::Text(text)) = event else {
        return false;
    };
    let expected = match key {
        egui::Key::A => "a",
        egui::Key::C => "c",
        egui::Key::V => "v",
        egui::Key::X => "x",
        egui::Key::Y => "y",
        egui::Key::Z => "z",
        _ => return false,
    };
    text.eq_ignore_ascii_case(expected)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key_event(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    #[test]
    fn repairs_missing_command_on_select_all() {
        let mut input = egui::RawInput {
            events: vec![key_event(egui::Key::A, egui::Modifiers::NONE)],
            ..default()
        };
        let command = egui::Modifiers {
            mac_cmd: true,
            command: true,
            ..default()
        };

        assert!(normalize_events(&mut input, command).is_empty());
        let egui::Event::Key { modifiers, .. } = input.events[0] else {
            panic!("expected a key event");
        };
        assert!(modifiers.command);
        assert!(modifiers.mac_cmd);
    }

    #[test]
    fn repaired_command_a_selects_the_whole_text_edit() {
        let ctx = egui::Context::default();
        let mut text = "not selected".to_owned();

        ctx.begin_frame(egui::RawInput::default());
        egui::CentralPanel::default().show(&ctx, |ui| {
            ui.text_edit_singleline(&mut text).request_focus();
        });
        let _ = ctx.end_frame();

        let command = egui::Modifiers {
            mac_cmd: true,
            command: true,
            ..default()
        };
        let mut input = egui::RawInput {
            events: vec![
                // This is the real order bevy_egui produces when it misses
                // the command modifier.
                egui::Event::Text("a".to_owned()),
                key_event(egui::Key::A, egui::Modifiers::NONE),
                egui::Event::Text("replacement".to_owned()),
            ],
            ..default()
        };
        normalize_events(&mut input, command);

        ctx.begin_frame(input);
        egui::CentralPanel::default().show(&ctx, |ui| {
            ui.text_edit_singleline(&mut text);
        });
        let _ = ctx.end_frame();

        assert_eq!(text, "replacement");
    }

    #[test]
    fn removes_text_emitted_for_a_missed_command_shortcut() {
        let command = egui::Modifiers {
            mac_cmd: true,
            command: true,
            ..default()
        };
        let mut input = egui::RawInput {
            events: vec![
                egui::Event::Text("c".to_owned()),
                key_event(egui::Key::C, egui::Modifiers::NONE),
            ],
            ..default()
        };

        assert_eq!(
            normalize_events(&mut input, command),
            vec![MissingClipboardEvent::Copy]
        );
        assert_eq!(input.events.len(), 1);
        assert!(matches!(input.events[0], egui::Event::Key { .. }));
    }

    #[test]
    fn requests_cut_only_when_bevy_egui_missed_command() {
        let command = egui::Modifiers {
            mac_cmd: true,
            command: true,
            ..default()
        };
        let mut missed = egui::RawInput {
            events: vec![key_event(egui::Key::X, egui::Modifiers::NONE)],
            ..default()
        };
        let mut handled = egui::RawInput {
            events: vec![key_event(egui::Key::X, command)],
            ..default()
        };

        assert_eq!(
            normalize_events(&mut missed, command),
            vec![MissingClipboardEvent::Cut]
        );
        assert!(normalize_events(&mut handled, command).is_empty());
    }
}
