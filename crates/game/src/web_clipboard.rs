//! Browser clipboard integration for egui text fields.
//!
//! `bevy_egui`'s clipboard feature cannot be enabled with this project's
//! locked Web API bindings, so paste events are collected here and appended
//! to egui's raw input before each frame begins.

use std::cell::RefCell;

use bevy::prelude::*;
use bevy_egui::{egui, EguiInput, EguiSet};
use wasm_bindgen::{closure::Closure, JsCast};

thread_local! {
    static PASTED_TEXT: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

pub struct WebClipboardPlugin;

impl Plugin for WebClipboardPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, install_paste_listener)
            .add_systems(
                PreUpdate,
                inject_pasted_text
                    .after(EguiSet::ProcessInput)
                    .before(EguiSet::BeginFrame),
            );
    }
}

fn install_paste_listener() {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        warn!("cannot install the browser paste listener: document is unavailable");
        return;
    };

    let listener =
        Closure::<dyn FnMut(web_sys::ClipboardEvent)>::new(|event: web_sys::ClipboardEvent| {
            let Some(clipboard) = event.clipboard_data() else {
                warn!("browser paste event did not contain clipboard data");
                return;
            };

            match clipboard.get_data("text/plain") {
                Ok(text) => {
                    event.prevent_default();
                    PASTED_TEXT.with(|pending| pending.borrow_mut().push(text));
                }
                Err(error) => warn!("could not read text from browser paste event: {error:?}"),
            }
        });

    if let Err(error) =
        document.add_event_listener_with_callback("paste", listener.as_ref().unchecked_ref())
    {
        warn!("cannot install the browser paste listener: {error:?}");
        return;
    }

    // The document owns the listener for the lifetime of the page.
    listener.forget();

    // Winit normally prevents every canvas keydown's browser default, which
    // also suppresses the `paste` event. Intercept only the paste shortcut in
    // the capture phase: stopping propagation keeps it away from Winit while
    // leaving the browser default enabled so the listener above receives it.
    let shortcut_listener =
        Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(|event: web_sys::KeyboardEvent| {
            if event.key().eq_ignore_ascii_case("v") && (event.meta_key() || event.ctrl_key()) {
                event.stop_propagation();
            }
        });
    if let Err(error) = document.add_event_listener_with_callback_and_bool(
        "keydown",
        shortcut_listener.as_ref().unchecked_ref(),
        true,
    ) {
        warn!("cannot install the browser paste-shortcut listener: {error:?}");
        return;
    }
    shortcut_listener.forget();
}

fn inject_pasted_text(mut inputs: Query<&mut EguiInput>) {
    let pasted = PASTED_TEXT.with(|pending| pending.take());
    if pasted.is_empty() {
        return;
    }

    let Ok(mut input) = inputs.get_single_mut() else {
        return;
    };
    input
        .events
        .extend(pasted.into_iter().map(egui::Event::Text));
}
