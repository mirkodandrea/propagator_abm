//! Graphics quality: what the renderer may spend on a machine weaker than the
//! one the scene was tuned on.
//!
//! The game is GPU-bound (the CPU idles while the render thread waits on the
//! frame), and the cost is the vegetation drawn once per pass — depth
//! prepass, main pass, every shadow cascade — plus full-screen effects
//! (depth of field, bloom, MSAA on an HDR target). Each knob here removes a
//! pass or shrinks one; none changes the simulation, the picking or what the
//! UI shows.
//!
//! `KIOSK_QUALITY=alta|media|bassa` picks a preset (default `alta`, the look
//! as authored). `KIOSK_GFX` then overrides single knobs, for tuning on the
//! kiosk machine: `KIOSK_GFX=-dof,-bloom,msaa=1,cascades=2,shadow=1500,veg=0.3,lod=800`.

use bevy::prelude::*;

#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct Graphics {
    /// Depth of field (and the depth prepass that only it needs).
    pub dof: bool,
    pub bloom: bool,
    /// MSAA samples: 4 or 1 (off).
    pub msaa: u32,
    /// Sun shadows at all.
    pub shadows: bool,
    /// Shadow cascades and how far they reach, in metres.
    pub cascades: usize,
    pub shadow_distance: f32,
    /// Shadow map texels per cascade side.
    pub shadow_map: usize,
    /// Share of the authored plant densities that is planted.
    pub vegetation: f32,
    /// Distance from the camera (metres) beyond which plants are drawn as
    /// proxies of a couple of dozen faces (see `vegetation`).
    pub lod_m: f32,
}

impl Graphics {
    /// As authored: what the M4 Pro this was tuned on draws at 35–45 FPS.
    pub const ALTA: Graphics = Graphics {
        dof: true,
        bloom: true,
        msaa: 4,
        shadows: true,
        cascades: 4,
        shadow_distance: 4000.0,
        shadow_map: 2048,
        vegetation: if cfg!(target_arch = "wasm32") { 0.12 } else { 0.5 },
        lod_m: 1400.0,
    };
    pub const MEDIA: Graphics = Graphics {
        dof: false,
        msaa: 4,
        cascades: 3,
        shadow_distance: 2500.0,
        vegetation: if cfg!(target_arch = "wasm32") { 0.12 } else { 0.35 },
        lod_m: 1000.0,
        ..Graphics::ALTA
    };
    pub const BASSA: Graphics = Graphics {
        dof: false,
        bloom: false,
        msaa: 1,
        cascades: 2,
        shadow_distance: 1800.0,
        shadow_map: 1024,
        vegetation: if cfg!(target_arch = "wasm32") { 0.08 } else { 0.2 },
        lod_m: 600.0,
        ..Graphics::ALTA
    };

    /// The preset from `KIOSK_QUALITY`, then the `KIOSK_GFX` overrides, then
    /// the old `KIOSK_VEG_DENSITY` (which still wins for the vegetation).
    pub fn from_env() -> Graphics {
        let mut g = match std::env::var("KIOSK_QUALITY").as_deref() {
            Ok("bassa" | "low") => Graphics::BASSA,
            Ok("media" | "medium") => Graphics::MEDIA,
            _ => Graphics::ALTA,
        };
        if let Ok(spec) = std::env::var("KIOSK_GFX") {
            g.apply(&spec);
        }
        if let Some(v) = std::env::var("KIOSK_VEG_DENSITY").ok().and_then(|v| v.parse().ok()) {
            g.vegetation = v;
        }
        g
    }

    fn apply(&mut self, spec: &str) {
        for item in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let (on, key) = match item.strip_prefix('-') {
                Some(k) => (false, k),
                None => (true, item.strip_prefix('+').unwrap_or(item)),
            };
            let (key, value) = key.split_once('=').unwrap_or((key, ""));
            let num = value.parse::<f32>().ok();
            match (key, num) {
                ("dof", _) => self.dof = on,
                ("bloom", _) => self.bloom = on,
                ("shadows", _) => self.shadows = on,
                ("lod", Some(m)) => self.lod_m = m.max(0.0),
                ("msaa", Some(n)) => self.msaa = if n >= 4.0 { 4 } else { 1 },
                ("msaa", None) => self.msaa = if on { 4 } else { 1 },
                ("cascades", Some(n)) => self.cascades = (n as usize).clamp(1, 4),
                ("shadow", Some(m)) => self.shadow_distance = m.max(100.0),
                ("shadow_map", Some(n)) => self.shadow_map = (n as usize).clamp(256, 4096),
                ("veg", Some(v)) => self.vegetation = v.max(0.0),
                _ => warn!("KIOSK_GFX: «{item}» non riconosciuto"),
            }
        }
    }

    pub fn msaa(&self) -> Msaa {
        // WebGL2 has no multisampled HDR targets.
        if cfg!(target_arch = "wasm32") || self.msaa < 4 { Msaa::Off } else { Msaa::Sample4 }
    }
}
