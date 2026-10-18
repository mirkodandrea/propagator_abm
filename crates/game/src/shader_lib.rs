//! WGSL modules shared between the game's shaders.

use bevy::asset::load_internal_asset;
use bevy::prelude::*;

const NOISE_SHADER_HANDLE: Handle<Shader> =
    Handle::weak_from_u128(0x2D6A_C0F3_5E19_4B87_8A41_7C03_E9B5_1D62);

const SCENE_LIGHT_SHADER_HANDLE: Handle<Shader> =
    Handle::weak_from_u128(0x5B1E_93A7_C4D2_4F08_B6E3_1A7D_28C9_F054);

/// Registers `rocca::noise` (`shaders/noise.wgsl`) and `rocca::scene_light`. Shaders `#import` it by
/// path, so the order of the plugins does not matter.
pub struct ShaderLibPlugin;

impl Plugin for ShaderLibPlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, NOISE_SHADER_HANDLE, "shaders/noise.wgsl", Shader::from_wgsl);
        load_internal_asset!(app, SCENE_LIGHT_SHADER_HANDLE, "shaders/scene_light.wgsl", Shader::from_wgsl);
    }
}
