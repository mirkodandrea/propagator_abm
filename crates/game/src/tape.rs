//! The material of the closed roads: hazard tape, animated in the shader
//! (`shaders/tape.wgsl`). Laid on the road (a little above it, like the road's
//! own surface), not floating: no light, no fog, hidden by what stands in front.

use bevy::asset::load_internal_asset;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderRef};

const TAPE_SHADER_HANDLE: Handle<Shader> = Handle::weak_from_u128(0x7A3E_51C2_0B94_4D6F_8E17_C2A9_55B0_1D83);

#[derive(Asset, AsBindGroup, TypePath, Clone)]
pub struct TapeMaterial {
    /// x = 1: the soft variant, a thin dashed line for the part of a closed
    /// stretch that vehicles can still drive.
    #[uniform(0)]
    pub mode: Vec4,
}

impl TapeMaterial {
    /// Closed piece: full hazard tape.
    pub fn closed() -> Self {
        TapeMaterial { mode: Vec4::ZERO }
    }

    /// The rest of a closed stretch: thin, dashed, dim.
    pub fn open_part() -> Self {
        TapeMaterial { mode: Vec4::new(1.0, 0.0, 0.0, 0.0) }
    }
}

impl Material for TapeMaterial {
    fn vertex_shader() -> ShaderRef {
        TAPE_SHADER_HANDLE.into()
    }
    fn fragment_shader() -> ShaderRef {
        TAPE_SHADER_HANDLE.into()
    }
    fn depth_bias(&self) -> f32 {
        1.0
    }
    fn specialize(
        _: &bevy::pbr::MaterialPipeline<Self>,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _: &bevy::render::mesh::MeshVertexBufferLayoutRef,
        _: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        Ok(())
    }
}

pub struct TapePlugin;

impl Plugin for TapePlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, TAPE_SHADER_HANDLE, "shaders/tape.wgsl", Shader::from_wgsl);
        app.add_plugins(MaterialPlugin::<TapeMaterial> { prepass_enabled: false, shadows_enabled: false, ..default() });
    }
}
