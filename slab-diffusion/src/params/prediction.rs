// prediction parameters must keep code order
#[rustfmt::skip]
use slab_diffusion_sys::{
    prediction_t_EPS_PRED,
    prediction_t_V_PRED,
    prediction_t_EDM_V_PRED,
    prediction_t_FLOW_PRED,
    prediction_t_FLUX_FLOW_PRED,
    prediction_t_FLUX2_FLOW_PRED,
    prediction_t_PREDICTION_COUNT,
};
use slab_diffusion_sys::prediction_t;

#[cfg_attr(any(not(windows), target_env = "gnu"), repr(u32))] // include windows-gnu
#[cfg_attr(all(windows, not(target_env = "gnu")), repr(i32))] // msvc being *special* again
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Prediction {
    Eps = prediction_t_EPS_PRED,
    V = prediction_t_V_PRED,
    EdmV = prediction_t_EDM_V_PRED,
    Flow = prediction_t_FLOW_PRED,
    FluxFlow = prediction_t_FLUX_FLOW_PRED,
    Flux2Flow = prediction_t_FLUX2_FLOW_PRED,
    Unknown = prediction_t_PREDICTION_COUNT,
}

impl From<Prediction> for prediction_t {
    fn from(value: Prediction) -> Self {
        value as Self
    }
}