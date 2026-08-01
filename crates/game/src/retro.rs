//! GPU-side material for the flat dev-scene look.

use bevy::asset::load_internal_asset;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderRef};

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

pub struct RetroShaderPlugin;

impl Plugin for RetroShaderPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            RETRO_SHADER_HANDLE,
            "shaders/retro.wgsl",
            Shader::from_wgsl
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
        },
    }
}
