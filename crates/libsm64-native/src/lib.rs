//! Small, main-thread-only owner of libsm64's global state. No ROM is embedded.
#![cfg(not(target_arch = "wasm32"))]

use std::{
    marker::PhantomData,
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
};

pub const TEXTURE_WIDTH: u32 = 704;
pub const TEXTURE_HEIGHT: u32 = 64;
pub const AUDIO_RATE: u32 = 32_000;
// Two synthesized blocks, at most 544 stereo frames each.
pub const AUDIO_BUFFER_SAMPLES: usize = 544 * 2 * 2;
const VERTICES: usize = 1024 * 3;
const MARIO_WING_CAP: u32 = 0x00000008;
static IN_USE: AtomicBool = AtomicBool::new(false);

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Surface {
    pub kind: i16,
    pub force: i16,
    pub terrain: u16,
    pub vertices: [[i32; 3]; 3],
}

#[repr(C)]
#[derive(Default)]
pub struct Input {
    pub camera_x: f32,
    pub camera_z: f32,
    pub stick_x: f32,
    pub stick_y: f32,
    pub jump: u8,
    pub attack: u8,
    pub crouch: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct State {
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub face_angle: f32,
    pub forward_velocity: f32,
    pub health: i16,
    pub action: u32,
    pub animation: i32,
    pub animation_frame: i16,
    pub flags: u32,
    pub particles: u32,
    pub invincibility: i16,
}

#[repr(C)]
struct Buffers {
    position: *mut f32,
    normal: *mut f32,
    color: *mut f32,
    uv: *mut f32,
    triangles: u16,
}

extern "C" {
    fn sm64_global_init(rom: *const u8, texture: *mut u8);
    fn sm64_global_terminate();
    fn sm64_audio_init(rom: *const u8);
    fn sm64_audio_set_active(active: u8);
    fn sm64_audio_reset();
    fn sm64_audio_tick(queued: u32, desired: u32, buffer: *mut i16) -> u32;
    fn sm64_play_music(player: u8, sequence: u16, fade: u16);
    fn sm64_stop_background_music(sequence: u16);
    #[cfg(test)]
    fn sm64_get_current_background_music() -> u16;
    fn sm64_static_surfaces_load(surfaces: *const Surface, count: u32);
    fn sm64_mario_create(x: f32, y: f32, z: f32) -> i32;
    fn sm64_mario_delete(id: i32);
    fn sm64_mario_tick(id: i32, input: *const Input, state: *mut State, buffers: *mut Buffers);
    fn sm64_set_mario_position(id: i32, x: f32, y: f32, z: f32);
    fn sm64_set_mario_water_level(id: i32, level: i32);
    fn sm64_set_mario_action(id: i32, action: u32);
    fn sm64_set_mario_velocity(id: i32, x: f32, y: f32, z: f32);
    fn sm64_set_mario_invincibility(id: i32, timer: i16);
    fn sm64_set_mario_state(id: i32, flags: u32);
    fn sm64_mario_take_damage(id: i32, damage: u32, subtype: u32, x: f32, y: f32, z: f32);
    fn sm64_mario_attack(id: i32, x: f32, y: f32, z: f32, height: f32) -> bool;
    fn sm64_surface_find_floor_height(x: f32, y: f32, z: f32) -> f32;
}

pub struct Engine {
    id: Option<i32>,
    pub state: State,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub vertex_count: usize,
    audio_initialized: bool,
    audio_active: bool,
    flight_music: bool,
    // libsm64's globals may only be accessed by their owner on one thread.
    _main_thread: PhantomData<Rc<()>>,
}

impl Engine {
    pub fn new(rom: &[u8]) -> Result<(Self, Vec<u8>), &'static str> {
        // libsm64 trusts ROM offsets and compressed data. Reject unknown ROMs
        // before entering C, including byte-swapped and modified images.
        if rom.len() != 8 * 1024 * 1024
            || blake3::hash(rom).to_hex().as_str()
                != "670d517ddaac88a517468a6466c8a6d08aa9901c0b235ae6081498e7c296c770"
        {
            return Err("Serve la ROM originale Super Mario 64 (USA), formato .z64.");
        }
        if IN_USE.swap(true, Ordering::SeqCst) {
            return Err("libsm64 è già in uso.");
        }
        let mut atlas = vec![0; TEXTURE_WIDTH as usize * TEXTURE_HEIGHT as usize * 4];
        unsafe {
            sm64_global_init(rom.as_ptr(), atlas.as_mut_ptr());
        }
        Ok((
            Self {
                id: None,
                state: State::default(),
                positions: vec![[0.; 3]; VERTICES],
                normals: vec![[0.; 3]; VERTICES],
                colors: vec![[0.; 3]; VERTICES],
                uvs: vec![[0.; 2]; VERTICES],
                vertex_count: 0,
                audio_initialized: false,
                audio_active: false,
                flight_music: false,
                _main_thread: PhantomData,
            },
            atlas,
        ))
    }

    pub fn init_audio(&mut self, rom: &[u8]) -> Result<(), &'static str> {
        if self.audio_initialized { return Ok(()); }
        if rom.len() != 8 * 1024 * 1024 || blake3::hash(rom).to_hex().as_str()
            != "670d517ddaac88a517468a6466c8a6d08aa9901c0b235ae6081498e7c296c770" {
            return Err("ROM audio non valida.");
        }
        unsafe { sm64_audio_init(rom.as_ptr()); }
        self.audio_initialized = true;
        self.audio_active = true;
        Ok(())
    }

    pub fn set_audio_active(&mut self, active: bool) {
        if self.audio_initialized {
            unsafe { sm64_audio_set_active(u8::from(active)); }
            self.audio_active = active;
            self.flight_music = false;
        }
    }

    /// Called only by the engine owner on the main thread. Audio playback
    /// consumes copied PCM; it never calls libsm64 from the audio thread.
    pub fn audio_tick(&mut self, queued_frames: usize, buffer: &mut [i16; AUDIO_BUFFER_SAMPLES]) -> usize {
        if !self.audio_active { return 0; }
        let flying = self.state.action == 0x10880899; // ACT_FLYING
        if flying != self.flight_music {
            unsafe {
                if flying { sm64_play_music(0, 0x040E, 0); } // Wing Cap / power-up theme
                else { sm64_stop_background_music(0x0E); }
            }
            self.flight_music = flying;
        }
        unsafe { sm64_audio_tick(queued_frames as u32, 1600, buffer.as_mut_ptr()) as usize * 4 }
    }

    pub fn load_surfaces(&mut self, surfaces: &[Surface]) {
        unsafe {
            sm64_static_surfaces_load(surfaces.as_ptr(), surfaces.len().try_into().unwrap());
        }
    }

    pub fn floor_height(&self, p: [f32; 3]) -> f32 {
        unsafe { sm64_surface_find_floor_height(p[0], p[1], p[2]) }
    }

    pub fn spawn(&mut self, p: [f32; 3]) -> bool {
        self.despawn();
        let id = unsafe { sm64_mario_create(p[0], p[1], p[2]) };
        if id < 0 {
            return false;
        }
        self.id = Some(id);
        self.state = State {
            position: p,
            health: 0x880,
            ..Default::default()
        };
        self.vertex_count = 0;
        true
    }

    pub fn despawn(&mut self) {
        // Reset pending sounds before freeing their Mario-owned source pointer.
        if self.audio_active { unsafe { sm64_audio_reset(); } }
        self.flight_music = false;
        if let Some(id) = self.id.take() {
            unsafe {
                sm64_mario_delete(id);
            }
        }
        self.vertex_count = 0;
    }

    pub fn tick(&mut self, input: &Input) {
        let Some(id) = self.id else { return };
        let mut buffers = Buffers {
            position: self.positions.as_mut_ptr().cast(),
            normal: self.normals.as_mut_ptr().cast(),
            color: self.colors.as_mut_ptr().cast(),
            uv: self.uvs.as_mut_ptr().cast(),
            triangles: 0,
        };
        unsafe {
            sm64_mario_tick(id, input, &mut self.state, &mut buffers);
        }
        self.vertex_count = buffers.triangles as usize * 3;
    }

    pub fn position(&mut self, p: [f32; 3]) {
        if let Some(id) = self.id {
            unsafe {
                sm64_set_mario_position(id, p[0], p[1], p[2]);
            }
        }
        self.state.position = p;
    }

    pub fn water_level(&mut self, level: i32) {
        if let Some(id) = self.id {
            unsafe {
                sm64_set_mario_water_level(id, level);
            }
        }
    }

    /// Toggle the native cap flag without starting a countdown. A fresh Mario
    /// has capTimer = 0, so libsm64 keeps the cap until this flag is cleared.
    /// Wing geometry and flight physics remain entirely native.
    pub fn set_wing_cap(&mut self, enabled: bool) {
        if let Some(id) = self.id {
            let flags = if enabled {
                self.state.flags | MARIO_WING_CAP | 0x11 // normal cap, on head
            } else {
                self.state.flags & !MARIO_WING_CAP
            };
            unsafe { sm64_set_mario_state(id, flags) };
            self.state.flags = flags;
        }
    }

    pub fn has_wing_cap(&self) -> bool {
        self.state.flags & MARIO_WING_CAP != 0
    }

    pub fn hurt(&mut self, source: [f32; 3], fire: bool) {
        let Some(id) = self.id else { return };
        if self.state.invincibility > 0 {
            return;
        }
        unsafe {
            sm64_mario_take_damage(
                id,
                if fire { 2 } else { 1 },
                0,
                source[0],
                source[1],
                source[2],
            );
            if fire {
                sm64_set_mario_action(id, 0x010208B4); // ACT_BURNING_JUMP
                sm64_set_mario_velocity(id, 0., 45., 0.);
            }
            sm64_set_mario_invincibility(id, 60);
        }
    }

    pub fn attacking(&mut self, p: [f32; 3], height: f32) -> bool {
        self.id
            .is_some_and(|id| unsafe { sm64_mario_attack(id, p[0], p[1], p[2], height) })
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        unsafe {
            sm64_global_terminate();
        }
        IN_USE.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_physics_with_local_rom() {
        assert!(Engine::new(&[0; 16]).is_err());
        let Ok(rom) = std::fs::read("../../data/mario64/Super Mario 64 (USA).z64") else {
            eprintln!("Skipping ROM-dependent physics check: local ROM unavailable.");
            return;
        };
        let (mut engine, texture) = Engine::new(&rom).unwrap();
        assert_eq!(texture.len(), 704 * 64 * 4);
        assert!(texture.iter().any(|&b| b != 0));
        assert!(
            Engine::new(&rom).is_err(),
            "C globals have exactly one owner"
        );
        let floor = [
            Surface {
                vertices: [[-8000, 0, -8000], [-8000, 0, 8000], [8000, 0, -8000]],
                ..Default::default()
            },
            Surface {
                vertices: [[8000, 0, -8000], [-8000, 0, 8000], [8000, 0, 8000]],
                ..Default::default()
            },
        ];
        engine.load_surfaces(&floor);
        assert!(engine.spawn([0., 200., 0.]));
        engine.water_level(-10000);
        let neutral = Input {
            camera_z: -1.,
            ..Default::default()
        };
        for _ in 0..90 {
            engine.tick(&neutral);
        }
        assert!(engine.state.position[1].abs() < 1.);
        assert!(engine.vertex_count > 0 && engine.vertex_count <= VERTICES);
        engine.set_wing_cap(true);
        for _ in 0..2100 {
            engine.tick(&neutral);
        }
        assert!(engine.has_wing_cap(), "Wing Cap lasts beyond 60 seconds");
        assert!(engine.vertex_count > 0 && engine.vertex_count <= VERTICES);
        engine.set_wing_cap(false);
        engine.tick(&neutral);
        assert!(!engine.has_wing_cap(), "user can disable Wing Cap");
        engine.set_wing_cap(true);
        assert!(engine.spawn([0., 200., 0.]));
        engine.tick(&neutral);
        assert!(!engine.has_wing_cap(), "respawn starts without Wing Cap");
        for _ in 0..90 {
            engine.tick(&neutral);
        }
        assert!(engine.positions[..engine.vertex_count]
            .iter()
            .flatten()
            .all(|x| x.is_finite()));
        let walk = Input {
            camera_z: -1.,
            stick_y: -1.,
            ..Default::default()
        };
        for _ in 0..30 {
            engine.load_surfaces(&floor);
            engine.tick(&walk);
        }
        assert!(
            engine.state.position[2] < -100.,
            "forward moves away from the camera: {:?}",
            engine.state
        );
        let jump = Input {
            camera_z: -1.,
            jump: 1,
            ..Default::default()
        };
        for _ in 0..4 {
            engine.tick(&jump);
        }
        assert!(engine.state.position[1] > 30.);
        let before = engine.state.health;
        engine.hurt([0., 0., 0.], true);
        for _ in 0..20 {
            engine.tick(&neutral);
        }
        assert!(engine.state.health < before, "fire causes real SM64 damage");
        assert!(engine.spawn([0., 200., 0.]));
        engine.water_level(600);
        for _ in 0..45 {
            engine.tick(&neutral);
        }
        assert_ne!(
            engine.state.action & (1 << 13),
            0,
            "Mario swims below the waterline"
        );
        engine.init_audio(&rom).unwrap();
        assert!(engine.spawn([0., 2000., 0.]));
        engine.water_level(-10000);
        engine.set_wing_cap(true);
        unsafe {
            sm64_set_mario_action(engine.id.unwrap(), 0x10880899);
            sm64_set_mario_velocity(engine.id.unwrap(), 0., 0., 40.);
        }
        engine.tick(&neutral);
        assert_eq!(engine.state.action, 0x10880899);
        let mut pcm = [0; AUDIO_BUFFER_SAMPLES];
        let mut audible = false;
        for _ in 0..30 {
            let count = engine.audio_tick(0, &mut pcm);
            assert_eq!(count, AUDIO_BUFFER_SAMPLES);
            audible |= pcm[..count].iter().any(|&s| s != 0);
        }
        assert!(audible, "ROM music produces actual PCM samples");
        assert_eq!(unsafe { sm64_get_current_background_music() } & 0xFF, 0x0E);
        assert!(engine.spawn([0., 200., 0.]));
        engine.tick(&neutral);
        engine.audio_tick(0, &mut pcm);
        assert!(!engine.flight_music, "respawn stops flight music");
        assert_eq!(unsafe { sm64_get_current_background_music() }, 0xFFFF);
        engine.set_audio_active(false);
        assert_eq!(engine.audio_tick(0, &mut pcm), 0);
        engine.set_audio_active(true);
        assert_eq!(engine.audio_tick(0, &mut pcm), AUDIO_BUFFER_SAMPLES);
        engine.despawn();
        engine.load_surfaces(&[]);
        drop(engine);
        assert!(
            Engine::new(&rom).is_ok(),
            "cleanup releases global ownership"
        );
    }
}
