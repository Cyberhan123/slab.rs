/// VAE tiling parameters.
use slab_diffusion_sys::sd_tiling_params_t;

/// Rust mirror of `sd_tiling_params_t`.
#[derive(Debug, Clone)]
pub struct TilingParams {
    pub enabled: bool,
    pub tile_size_x: i32,
    pub tile_size_y: i32,
    pub target_overlap: f32,
    pub rel_size_x: f32,
    pub rel_size_y: f32,
}

impl From<TilingParams> for sd_tiling_params_t {
    fn from(value: TilingParams) -> Self {
        Self {
            enabled: value.enabled,
            tile_size_x: value.tile_size_x,
            tile_size_y: value.tile_size_y,
            target_overlap: value.target_overlap,
            rel_size_x: value.rel_size_x,
            rel_size_y: value.rel_size_y,
        }
    }
}
