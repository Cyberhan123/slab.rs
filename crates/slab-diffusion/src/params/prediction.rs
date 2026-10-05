use serde::{Deserialize, Serialize};

// prediction parameters must keep code order
#[rustfmt::skip]
use slab_diffusion_sys::{
    prediction_t_EPS_PRED,
    prediction_t_V_PRED,
    prediction_t_EDM_V_PRED,
    prediction_t_FLOW_PRED,
    prediction_t_FLUX_FLOW_PRED,
    prediction_t_SEFI_FLOW_PRED,
    prediction_t_MINIT2I_FLOW_PRED,
    prediction_t_PREDICTION_COUNT,
};
use slab_diffusion_sys::prediction_t;

// slab-build-utils normalizes bindgen enum aliases to c_int, so the
// discriminant consts are i32 on every target.
#[repr(i32)]
#[derive(Debug, Copy, Clone, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Prediction {
    Eps = prediction_t_EPS_PRED,
    V = prediction_t_V_PRED,
    EdmV = prediction_t_EDM_V_PRED,
    Flow = prediction_t_FLOW_PRED,
    FluxFlow = prediction_t_FLUX_FLOW_PRED,
    SefiFlow = prediction_t_SEFI_FLOW_PRED,
    MinitwoFlow = prediction_t_MINIT2I_FLOW_PRED,
    #[default]
    Unknown = prediction_t_PREDICTION_COUNT,
}

impl From<Prediction> for prediction_t {
    fn from(value: Prediction) -> Self {
        value as Self
    }
}
