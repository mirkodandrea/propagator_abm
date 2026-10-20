//! GPU-side material for the flat dev-scene look.

use bevy::asset::load_internal_asset;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderDefVal, ShaderRef};

const RETRO_SHADER_HANDLE: Handle<Shader> =
    Handle::weak_from_u128(0x91A7_44D2_6B0E_4C71_AA90_2201_88C4_2F10);

/// Extension of StandardMaterial so vertex colours, alpha modes, culling and
/// the existing PBR setup are retained. The effect stays entirely on the GPU
/// and therefore also covers merged vegetation and building meshes.
#[derive(Asset, AsBindGroup, TypePath, Clone, Default)]
pub struct RetroExtension {
    // A bare f32 uniform is 4 bytes, which WebGL2 rejects (bindings must be a
    // multiple of 16 bytes there; desktop backends don't enforce it). Keep all
    // controls in one vec4: enabled, edge strength, pulse strength, spare.
    #[uniform(100)]
    pub enabled: Vec4,
    /// Road distance field, for the terrain only (`enabled.w` = 1). Other
    /// materials leave it unset and Bevy binds a fallback texture.
    #[texture(101)]
    #[sampler(102)]
    pub road_mask: Option<Handle<Image>>,
    /// World metres to mask coordinates (`ground_mask::RoadMask::rect`).
    #[uniform(103)]
    pub mask_rect: Vec4,
    /// Meadow cover, one texel per fire cell (`ground_mask::grass_map`).
    #[texture(104)]
    #[sampler(105)]
    pub grass_map: Option<Handle<Image>>,
    /// Tactical view (`tactical.rs`): x = how far the colours are drawn
    /// toward grey, 0..1. Only terrain and foliage read it.
    #[uniform(106)]
    pub tactical: Vec4,
    /// Terrain only: the ground colour (sRGB), one texel per terrain sample,
    /// and the surface normal (Bevy frame), so the look does not depend on the
    /// mesh density (`terrain_mesh`). Unset elsewhere: Bevy binds a fallback.
    ///
    /// Not in the browser: WebGL2 has 16 texture units per stage and these two
    /// would be the 17th and 18th there. The terrain mesh carries the colour
    /// and the normal per vertex instead (`terrain_mesh`).
    #[cfg(not(target_arch = "wasm32"))]
    #[texture(107)]
    #[sampler(108)]
    pub ground_colour: Option<Handle<Image>>,
    #[cfg(not(target_arch = "wasm32"))]
    #[texture(109)]
    #[sampler(110)]
    pub ground_normal: Option<Handle<Image>>,
    /// World metres to those textures' uv: x, y = 1 / (posting * size),
    /// z = half a texel in u, w = the north edge plus half a texel in v.
    #[cfg(not(target_arch = "wasm32"))]
    #[uniform(111)]
    pub ground_rect: Vec4,
}

/// Per-material strength of the training-simulation finish.
///
/// Environment classes need different treatment: a readable building benefits
/// from a restrained rim, while applying that same rim to a hundred thousand
/// plants turns the terrain into cyan static.
#[derive(Debug, Clone, Copy)]
pub struct RetroStyle {
    pub edge: f32,
    pub pulse: f32,
}

impl RetroStyle {
    /// Ordinary interactive geometry: visible silhouette, barely moving fill.
    pub const STANDARD: RetroStyle = RetroStyle {
        edge: 0.55,
        pulse: 0.20,
    };
    /// Structural scenery such as buildings.
    pub const STRUCTURE: RetroStyle = RetroStyle {
        edge: 0.38,
        pulse: 0.08,
    };
    /// Routes already carry their hierarchy in width and colour.
    pub const ROUTE: RetroStyle = RetroStyle {
        edge: 0.12,
        pulse: 0.0,
    };
    /// Dense background geometry must stay matte and still.
    pub const BACKGROUND: RetroStyle = RetroStyle {
        edge: 0.0,
        pulse: 0.0,
    };
}

pub type RetroMaterial = ExtendedMaterial<StandardMaterial, RetroExtension>;

impl MaterialExtension for RetroExtension {
    fn fragment_shader() -> ShaderRef {
        RETRO_SHADER_HANDLE.into()
    }
}

/// `GROUND_TEX`: the terrain reads its colour and normal from textures
/// (native); without it, from the mesh's vertex colour and normal (browser).
fn shader_defs() -> Vec<ShaderDefVal> {
    #[cfg(not(target_arch = "wasm32"))]
    return vec![ShaderDefVal::Bool("GROUND_TEX".into(), true)];
    #[cfg(target_arch = "wasm32")]
    return Vec::new();
}

pub struct RetroShaderPlugin;

impl Plugin for RetroShaderPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            RETRO_SHADER_HANDLE,
            "shaders/retro.wgsl",
            Shader::from_wgsl_with_defs,
            shader_defs()
        );
        app.add_plugins(MaterialPlugin::<RetroMaterial>::default());
    }
}

/// Make a material with the standard dev-scene treatment.
pub fn material(base: StandardMaterial, dev: bool) -> RetroMaterial {
    material_with_style(base, dev, RetroStyle::STANDARD)
}

/// Make a material with a role-appropriate dev-scene treatment.
pub fn material_with_style(
    base: StandardMaterial,
    dev: bool,
    style: RetroStyle,
) -> RetroMaterial {
    RetroMaterial {
        base,
        extension: RetroExtension {
            enabled: Vec4::new(
                dev as u8 as f32,
                style.edge.clamp(0.0, 1.0),
                style.pulse.clamp(0.0, 1.0),
                0.0,
            ),
            ..default()
        },
    }
}

/// The terrain's material: the retro finish plus the road mask the shader
/// draws the road network from.
pub fn terrain_material(base: StandardMaterial, mask: Handle<Image>, rect: Vec4, colour: Handle<Image>, normal: Handle<Image>, ground_rect: Vec4) -> RetroMaterial {
    #[cfg(target_arch = "wasm32")]
    let _ = (colour, normal, ground_rect);
    let mut m = material_with_style(base, false, RetroStyle::BACKGROUND);
    m.extension.enabled.w = 1.0;
    m.extension.road_mask = Some(mask);
    m.extension.mask_rect = rect;
    m.extension.grass_map = Some(crate::ground_mask::GRASS_MAP);
    #[cfg(not(target_arch = "wasm32"))]
    {
        m.extension.ground_colour = Some(colour);
        m.extension.ground_normal = Some(normal);
        m.extension.ground_rect = ground_rect;
    }
    m
}

/// Surface finish of a structure's material (`enabled.w` of the shader).
#[derive(Debug, Clone, Copy)]
pub enum Finish {
    /// Town buildings, kit props and the pier: plaster, tiles, planks.
    Town = 3,
    /// Landmarks: dressed stone.
    Stone = 4,
    /// Boats: planking, deck boards, sail panels.
    Boat = 5,
    /// Bare limestone off the terrain (the rocca's crag, the coast's boulders):
    /// the terrain's own beds, cut from world height.
    Rock = 6,
}

/// A structure material with a procedural surface finish.
pub fn finished_material(base: StandardMaterial, style: RetroStyle, finish: Finish) -> RetroMaterial {
    let mut m = material_with_style(base, false, style);
    m.extension.enabled.w = finish as u8 as f32;
    m
}
