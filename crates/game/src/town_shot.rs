//! Screenshot dall'alto dei paesi, con lo stesso terreno, le stesse strade e
//! gli stessi edifici del gioco (moduli condivisi, nessuna copia), senza
//! simulazione, interfaccia, cielo né fuoco. Serve a migliorare il disegno dei
//! paesi (`tools/factory/town.py`) guardando quello che il visitatore vede.
//!
//! ```text
//! cargo run -p game --release --bin town_shot
//! TOWN_OUT=out/town_shot TOWN_ONLY='Castelvento+Fornaci' TOWN_HALF=450 TOWN_VEG=1 \
//!   TOWN_PITCH=-90 TOWN_YAW=0 cargo run -p game --release --bin town_shot
//! ```
//! `ROCCA_DATA` (default `data`), `TOWN_CASE` (default il primo del territorio),
//! `TOWN_SIZE` in pixel (default 1100), `TOWN_VEG=1` per gli alberi. Serve uno schermo.

use bevy::prelude::*;
use bevy::render::view::screenshot::ScreenshotManager;
use bevy::window::PrimaryWindow;

mod agents;
mod buildings;
mod field;
mod frame;
mod life;
mod models;
mod people;
mod quality;
mod retro;
mod roads;
mod sim;
mod terrain_mesh;
mod textures;
mod toy;
mod vegetation;
mod visual_scale;

use sim::Sim;

#[derive(Resource)]
struct Plan {
    out: String,
    /// (nome, centro in metri)
    shots: Vec<(String, Pos2)>,
    half: f32,
    pitch: f32,
    yaw: f32,
    veg: bool,
}

#[derive(Clone, Copy)]
struct Pos2(f32, f32);

fn env<T: std::str::FromStr>(k: &str, d: T) -> T {
    std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d)
}

fn main() -> anyhow::Result<()> {
    let data = std::env::var("ROCCA_DATA").unwrap_or_else(|_| "data".to_string());
    let probe = rocca::Territory::load(std::path::Path::new(&data))?;
    let case: String = std::env::var("TOWN_CASE").unwrap_or_else(|_| probe.playlist()[0].to_string());
    let sim = Sim::new(std::path::Path::new(&data), &case, 1)?;

    let only: Vec<String> = std::env::var("TOWN_ONLY").map(|v| v.split('+').map(str::to_string).collect()).unwrap_or_default();
    let mut shots = vec![];
    for d in &sim.districts {
        if !only.is_empty() && !only.contains(&d.name) {
            continue;
        }
        for (i, n) in d.nuclei.iter().enumerate() {
            let name = if d.nuclei.len() > 1 { format!("{}_{}", d.name, i + 1) } else { d.name.clone() };
            shots.push((name.replace(' ', "_"), Pos2(n.x, n.y)));
        }
        if d.nuclei.is_empty() {
            shots.push((d.name.replace(' ', "_"), Pos2(d.centre.x, d.centre.y)));
        }
    }
    eprintln!("town_shot: {} viste: {:?}", shots.len(), shots.iter().map(|s| s.0.as_str()).collect::<Vec<_>>());
    let size: f32 = env("TOWN_SIZE", 1100.0);
    let plan = Plan {
        out: std::env::var("TOWN_OUT").unwrap_or_else(|_| "out/town_shot".into()),
        shots,
        half: env("TOWN_HALF", 400.0),
        pitch: env("TOWN_PITCH", -90.0),
        yaw: env("TOWN_YAW", 0.0),
        veg: std::env::var("TOWN_VEG").is_ok(),
    };
    let _ = std::fs::create_dir_all(&plan.out);

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "town_shot".into(),
                resolution: bevy::window::WindowResolution::new(size, size).with_scale_factor_override(1.0),
                resizable: false,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(retro::RetroShaderPlugin)
        .insert_resource(sim)
        .insert_resource(plan)
        .insert_resource(quality::Graphics::from_env())
        .insert_resource(ClearColor(Color::srgb(0.55, 0.62, 0.68)))
        .insert_resource(AmbientLight { color: Color::WHITE, brightness: 700.0 })
        .add_systems(Startup, (setup, buildings::spawn))
        .add_systems(Startup, vegetation::spawn.run_if(|p: Res<Plan>| p.veg))
        .add_systems(Update, drive)
        .run();
    Ok(())
}

fn setup(
    mut commands: Commands,
    sim: Res<Sim>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<retro::RetroMaterial>>,
) {
    terrain_mesh::build(&sim.scn, &mut commands, &mut meshes, &mut materials);
    roads::build(&sim.scn, &mut commands, &mut meshes, &mut materials);
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight { shadows_enabled: true, illuminance: 9000.0, ..default() },
        transform: Transform::from_xyz(-1.0, 2.0, 0.8).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });
    commands.spawn((Camera3dBundle {
        projection: OrthographicProjection { scale: 1.0, far: 20_000.0, near: -20_000.0, ..default() }.into(),
        ..default()
    },));
}

/// Un'inquadratura alla volta: sposta la camera, aspetta che il mondo si
/// assesti, scatta; alla fine esce.
fn drive(
    time: Res<Time>,
    plan: Res<Plan>,
    sim: Res<Sim>,
    mut cam: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
    windows: Query<Entity, With<PrimaryWindow>>,
    mut mgr: ResMut<ScreenshotManager>,
    mut exit: EventWriter<AppExit>,
    mut at: Local<(usize, f32)>,
) {
    let Ok(window) = windows.get_single() else { return };
    let Some((name, Pos2(x, y))) = plan.shots.get(at.0).cloned() else {
        at.1 += time.delta_seconds();
        if at.1 > 0.5 {
            exit.send(AppExit::Success);
        }
        return;
    };
    let (p, h) = terrain_mesh::cell_ground(&sim.scn, sim.scn.world.cell_of(scenario::Pos { x, y }));
    let focus = frame::to_bevy(p, h);
    if let Ok((mut t, mut proj)) = cam.get_single_mut() {
        if let Projection::Orthographic(o) = &mut *proj {
            o.scale = 2.0 * plan.half / 1100.0;
        }
        let pitch = plan.pitch.to_radians();
        let yaw = plan.yaw.to_radians();
        let dir = Vec3::new(yaw.sin() * pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos()) * 3000.0;
        *t = Transform::from_translation(focus + dir).looking_at(focus, if plan.pitch <= -89.0 { Vec3::NEG_Z } else { Vec3::Y });
    }
    at.1 += time.delta_seconds();
    if at.1 > 2.0 {
        let _ = mgr.save_screenshot_to_disk(window, format!("{}/{}.png", plan.out, name));
        eprintln!("town_shot: {name}");
        at.0 += 1;
        at.1 = 0.0;
    }
}
