//! Browser text-edit integration for egui.
//!
//! `bevy_egui`'s clipboard feature cannot be enabled with this project's
//! locked Web API bindings. More importantly, winit prevents the browser's
//! default handling for canvas key events, which suppresses the DOM clipboard
//! events egui needs. This bridge keeps the browser in charge of clipboard
//! access and sends the resulting edit commands into egui before its frame.

use std::cell::RefCell;

use bevy::prelude::*;
use bevy_egui::{egui, EguiInput, EguiOutput, EguiSet};
use wasm_bindgen::{closure::Closure, JsCast};
use wasm_bindgen_futures::{spawn_local, JsFuture};

thread_local! {
    static PENDING_EVENTS: RefCell<Vec<egui::Event>> = const { RefCell::new(Vec::new()) };
}

pub struct WebClipboardPlugin;

impl Plugin for WebClipboardPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, install_browser_listeners)
            .add_systems(
                PreUpdate,
                inject_browser_events
                    .after(EguiSet::ProcessInput)
                    .before(EguiSet::BeginFrame),
            )
            .add_systems(PostUpdate, write_copied_text.after(EguiSet::ProcessOutput));
    }
}

fn install_browser_listeners() {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        warn!("cannot install the browser text-edit listeners: document is unavailable");
        return;
    };

    let paste_listener =
        Closure::<dyn FnMut(web_sys::ClipboardEvent)>::new(|event: web_sys::ClipboardEvent| {
            let Some(clipboard) = event.clipboard_data() else {
                warn!("browser paste event did not contain clipboard data");
                return;
            };

            match clipboard.get_data("text/plain") {
                Ok(text) => {
                    event.prevent_default();
                    queue(egui::Event::Paste(text));
                }
                Err(error) => warn!("could not read text from browser paste event: {error:?}"),
            }
        });
    if let Err(error) =
        document.add_event_listener_with_callback("paste", paste_listener.as_ref().unchecked_ref())
    {
        warn!("cannot install the browser paste listener: {error:?}");
        return;
    }
    paste_listener.forget();

    let copy_listener =
        Closure::<dyn FnMut(web_sys::ClipboardEvent)>::new(|_event: web_sys::ClipboardEvent| {
            queue(egui::Event::Copy);
        });
    if let Err(error) =
        document.add_event_listener_with_callback("copy", copy_listener.as_ref().unchecked_ref())
    {
        warn!("cannot install the browser copy listener: {error:?}");
        return;
    }
    copy_listener.forget();

    let cut_listener =
        Closure::<dyn FnMut(web_sys::ClipboardEvent)>::new(|_event: web_sys::ClipboardEvent| {
            queue(egui::Event::Cut);
        });
    if let Err(error) =
        document.add_event_listener_with_callback("cut", cut_listener.as_ref().unchecked_ref())
    {
        warn!("cannot install the browser cut listener: {error:?}");
        return;
    }
    cut_listener.forget();

    // Winit calls preventDefault for canvas keydowns. Clipboard shortcuts must
    // therefore stop before winit, so the browser can emit copy/cut/paste.
    // Select-all and undo/redo also come through here: on web, bevy_egui has to
    // infer macOS from the user agent, and a failed inference turns Cmd into a
    // non-command modifier. The DOM event already tells us exactly which
    // primary modifier was pressed.
    for event_name in ["keydown", "keyup"] {
        let listener =
            Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(|event: web_sys::KeyboardEvent| {
                let meta = event.meta_key();
                let ctrl = event.ctrl_key();
                if !meta && !ctrl {
                    return;
                }

                let key = event.key().to_ascii_lowercase();
                if matches!(key.as_str(), "c" | "x" | "v") {
                    // Do not prevent the default: it is what emits the DOM
                    // clipboard event handled above.
                    event.stop_propagation();
                    return;
                }

                let key = match key.as_str() {
                    "a" => egui::Key::A,
                    "z" => egui::Key::Z,
                    "y" => egui::Key::Y,
                    "arrowleft" => egui::Key::ArrowLeft,
                    "arrowright" => egui::Key::ArrowRight,
                    "arrowup" => egui::Key::ArrowUp,
                    "arrowdown" => egui::Key::ArrowDown,
                    "backspace" => egui::Key::Backspace,
                    "delete" => egui::Key::Delete,
                    "home" => egui::Key::Home,
                    "end" => egui::Key::End,
                    _ => return,
                };

                event.stop_propagation();
                event.prevent_default();
                queue(egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: event.type_() == "keydown",
                    repeat: event.repeat(),
                    modifiers: egui::Modifiers {
                        alt: event.alt_key(),
                        ctrl,
                        shift: event.shift_key(),
                        mac_cmd: meta,
                        command: meta || ctrl,
                    },
                });
            });
        if let Err(error) = document.add_event_listener_with_callback_and_bool(
            event_name,
            listener.as_ref().unchecked_ref(),
            true,
        ) {
            warn!("cannot install the browser {event_name} listener: {error:?}");
            return;
        }
        listener.forget();
    }
}

fn queue(event: egui::Event) {
    PENDING_EVENTS.with(|pending| pending.borrow_mut().push(event));
}

fn inject_browser_events(mut inputs: Query<&mut EguiInput>) {
    let events = PENDING_EVENTS.with(|pending| pending.take());
    if events.is_empty() {
        return;
    }

    let Ok(mut input) = inputs.get_single_mut() else {
        return;
    };
    input.events.extend(events);
}

/// Send text selected by an egui `Copy` or `Cut` event back to the real system
/// clipboard. `EguiOutput` is populated immediately before this system.
fn write_copied_text(outputs: Query<&EguiOutput>) {
    let Ok(output) = outputs.get_single() else {
        return;
    };
    let text = &output.platform_output.copied_text;
    if text.is_empty() {
        return;
    }

    let text = text.clone();
    spawn_local(async move {
        let Some(window) = web_sys::window() else {
            warn!("cannot copy text: window is unavailable");
            return;
        };
        let promise = window.navigator().clipboard().write_text(&text);
        if let Err(error) = JsFuture::from(promise).await {
            warn!("could not write text to the browser clipboard: {error:?}");
        }
    });
}
