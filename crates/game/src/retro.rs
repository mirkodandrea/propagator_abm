//! GPU-side material for the flat, animated dev-scene look.

use bevy::asset::load_internal_asset;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderRef};

const RETRO_SHADER_HANDLE: Handle<Shader> =
    Handle::weak_from_u128(0x91A7_44D2_6B0E_4C71_AA90_2201_88C4_2F10);

/// Extension of StandardMaterial so vertex colours, alpha modes, culling and
/// the existing PBR setup are retained. The animation itself stays entirely
/// on the GPU and therefore also covers merged vegetation and building meshes.
#[derive(Asset, AsBindGroup, TypePath, Clone, Default)]
pub struct RetroExtension {
    #[uniform(100)]
    pub enabled: f32,
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

/// Make a material that is animated only for dev scenarios.
pub fn material(base: StandardMaterial, dev: bool) -> RetroMaterial {
    RetroMaterial {
        base,
        extension: RetroExtension {
            enabled: dev as u8 as f32,
        },
    }
}
