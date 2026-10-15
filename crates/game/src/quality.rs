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
//!
//! The operator bar (F2) also picks the preset, live and saved in
//! `~/.rocca_ventosa/grafica.txt` (`KIOSK_QUALITY` still wins). Everything
//! changes at once except the share of plants planted, which is drawn when
//! the scene is built: it applies from the next launch.

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

    /// Browser budget, also on desktop: no post-processing or shadow passes.
    pub const WEB: Graphics = Graphics {
        shadows: false, shadow_map: 256, cascades: 1,
        vegetation: 0.025, lod_m: 0.0,
        ..Graphics::BASSA
    };

    /// The preset from `KIOSK_QUALITY`, then the `KIOSK_GFX` overrides, then
    /// the old `KIOSK_VEG_DENSITY` (which still wins for the vegetation).
    pub fn from_env() -> Graphics {
        if cfg!(target_arch = "wasm32") { return Graphics::WEB; }
        let name = std::env::var("KIOSK_QUALITY").ok().or_else(saved);
        let mut g = match name.as_deref() {
            Some("bassa" | "low") => Graphics::BASSA,
            Some("media" | "medium") => Graphics::MEDIA,
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

    /// The preset this is, if it is one (the operator bar's selection).
    pub fn level(&self) -> Option<&'static str> {
        // the plant density applies only at launch, so it is not compared
        let same = |p: &Graphics| Graphics { vegetation: self.vegetation, ..*p } == *self;
        [("alta", &Graphics::ALTA), ("media", &Graphics::MEDIA), ("bassa", &Graphics::BASSA)]
            .into_iter()
            .find(|(_, p)| same(p))
            .map(|(n, _)| n)
    }

    pub fn msaa(&self) -> Msaa {
        // WebGL2 has no multisampled HDR targets.
        if cfg!(target_arch = "wasm32") || self.msaa < 4 { Msaa::Off } else { Msaa::Sample4 }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn saved_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    std::path::Path::new(&home).join(".rocca_ventosa").join("grafica.txt")
}

#[cfg(not(target_arch = "wasm32"))]
fn saved() -> Option<String> {
    std::fs::read_to_string(saved_path()).ok().map(|s| s.trim().to_string())
}

#[cfg(target_arch = "wasm32")]
fn saved() -> Option<String> {
    web_sys::window()?.local_storage().ok()??.get_item("rocca_ventosa_grafica").ok()?
}

/// Remember the level for the next launch. A failure is reported, never fatal.
#[cfg(not(target_arch = "wasm32"))]
pub fn save(level: &str) -> Result<(), String> {
    let p = saved_path();
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("grafica: {e}"))?;
    }
    std::fs::write(&p, level).map_err(|e| format!("grafica: {e}"))
}

#[cfg(target_arch = "wasm32")]
pub fn save(level: &str) -> Result<(), String> {
    let s = web_sys::window().and_then(|w| w.local_storage().ok().flatten()).ok_or("grafica: il browser non salva dati")?;
    s.set_item("rocca_ventosa_grafica", level).map_err(|_| "grafica: salvataggio non riuscito".to_string())
}
