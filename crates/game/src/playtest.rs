//! Opt-in browser automation. Input still travels through winit/egui; this
//! bridge only exposes observations and controls the clock, never game orders.
use bevy_egui::egui;
use crate::{kiosk::Kiosk, sim::Sim};

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::*;
    use std::cell::RefCell;
    use wasm_bindgen::JsValue;
    thread_local! { static CONTROLS: RefCell<Vec<serde_json::Value>> = const { RefCell::new(Vec::new()) }; }
    pub fn bridge() -> Option<JsValue> {
        let w = web_sys::window()?;
        let v = js_sys::Reflect::get(&w, &"roccaPlaytest".into()).ok()?;
        (!v.is_undefined() && !v.is_null()).then_some(v)
    }
    pub fn number(key: &str) -> f64 {
        bridge().and_then(|b| js_sys::Reflect::get(&b, &key.into()).ok()).and_then(|v| v.as_f64()).unwrap_or(0.0).max(0.0)
    }
    pub fn set(key: &str, value: f64) {
        if let Some(b) = bridge() { let _ = js_sys::Reflect::set(&b, &key.into(), &value.into()); }
    }
    pub fn begin() { CONTROLS.with(|c| c.borrow_mut().clear()); }
    pub fn control(label: &str, r: &egui::Response, enabled: bool) {
        if bridge().is_none() { return; }
        CONTROLS.with(|c| c.borrow_mut().push(serde_json::json!({
            "label": label, "x": r.rect.center().x, "y": r.rect.center().y,
            "rect": [r.rect.min.x, r.rect.min.y, r.rect.max.x, r.rect.max.y],
            "enabled": enabled && r.enabled(), "hovered": r.hovered(),
            "pointer_down": r.ctx.input(|i| i.pointer.primary_down()),
        })));
    }
    pub fn publish(k: &Kiosk, sim: &Sim) {
        let Some(b) = bridge() else { return };
        let state = serde_json::json!({
            "phase": if k.title { "title".to_string() } else if k.intro { "intro".to_string() } else { format!("{:?}", k.phase) },
            "case": k.case, "time_s": sim.time_s(), "end_s": sim.case.duration_s(),
            "player": k.player, "error": k.error, "baseline_ready": k.baseline().is_some(),
            "tour": k.tour, "selected": k.selected, "saved": k.saved,
            "priorities": k.proposed.priorities,
            "proposed_civil": k.proposed.civil.iter().map(|c| format!("{c:?}")).collect::<Vec<_>>(), "confirm_evac": k.confirm_evac.map(|(d, _)| d),
            "crisis": k.crisis.as_ref().map(|c| format!("{:?}", c.kind)),
            "districts": sim.districts.iter().enumerate().map(|(d, district)| serde_json::json!({
                "name": district.name, "may_evacuate": sim.may_evacuate(d),
                "civil": format!("{:?}", sim.active.civil[d]),
            })).collect::<Vec<_>>(),
            "controls": CONTROLS.with(|c| c.borrow().clone()),
        });
        let value = js_sys::JSON::parse(&state.to_string()).expect("playtest state JSON");
        let _ = js_sys::Reflect::set(&b, &"state".into(), &value);
        set("frame", number("frame") + 1.0);
    }
}

pub fn active() -> bool {
    #[cfg(target_arch = "wasm32")] { browser::bridge().is_some() }
    #[cfg(not(target_arch = "wasm32"))] { false }
}
pub fn begin() {
    #[cfg(target_arch = "wasm32")] browser::begin();
}
pub fn control(label: &str, r: &egui::Response, enabled: bool) {
    #[cfg(target_arch = "wasm32")] browser::control(label, r, enabled);
    #[cfg(not(target_arch = "wasm32"))] let _ = (label, r, enabled);
}
/// In automation, countdowns advance only when the agent asks to elapse time.
pub fn delta(normal: f32) -> f32 {
    #[cfg(target_arch = "wasm32")]
    if active() {
        let dt = browser::number("elapse_s").min(3600.0) as f32;
        browser::set("elapse_s", 0.0);
        return dt;
    }
    normal
}
/// Spend CPU on fixed simulation steps, independent of wall time/FPS. Yield
/// periodically to render/input, and stop on the exact step raising a crisis.
pub fn tick(sim: &mut Sim, dt: f32) -> anyhow::Result<bool> {
    #[cfg(target_arch = "wasm32")]
    if active() {
        let mut remaining = browser::number("advance_s");
        if remaining == 0.0 && dt > 0.0 { return sim.tick(dt); }
        let start = js_sys::Date::now();
        let mut stepped = false;
        while remaining >= rocca::STEP_S as f64 && sim.crisis.is_none() && sim.time_s() < sim.case.duration_s() {
            let steps = sim.advance_steps(1)?;
            remaining -= steps as f64 * rocca::STEP_S as f64;
            stepped |= steps > 0;
            if js_sys::Date::now() - start >= 40.0 { break; }
        }
        if sim.crisis.is_some() || sim.time_s() >= sim.case.duration_s() || remaining < rocca::STEP_S as f64 {
            remaining = 0.0;
        }
        browser::set("advance_s", remaining);
        return Ok(stepped);
    }
    sim.tick(dt)
}
pub fn observe(kiosk: bevy::prelude::Res<Kiosk>, sim: bevy::prelude::Res<Sim>) {
    #[cfg(target_arch = "wasm32")] browser::publish(&kiosk, &sim);
    #[cfg(not(target_arch = "wasm32"))] let _ = (kiosk, sim);
}
