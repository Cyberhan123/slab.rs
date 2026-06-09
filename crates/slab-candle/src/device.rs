use candle_core::Device;
use slab_types::RuntimeDevicePreference;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CandleDeviceError {
    #[error("device preference '{preference}' is unavailable: {message}")]
    Unavailable { preference: RuntimeDevicePreference, message: String },
}

pub fn resolve_device(
    preference: Option<RuntimeDevicePreference>,
) -> Result<Device, CandleDeviceError> {
    match preference.unwrap_or_default() {
        RuntimeDevicePreference::Auto => resolve_auto_device(),
        RuntimeDevicePreference::Cpu => Ok(Device::Cpu),
        preference @ RuntimeDevicePreference::Cuda { ordinal } => Device::new_cuda(ordinal)
            .map_err(|error| CandleDeviceError::Unavailable {
                preference,
                message: error.to_string(),
            }),
        preference @ RuntimeDevicePreference::Metal { ordinal } => Device::new_metal(ordinal)
            .map_err(|error| CandleDeviceError::Unavailable {
                preference,
                message: error.to_string(),
            }),
    }
}

fn resolve_auto_device() -> Result<Device, CandleDeviceError> {
    let cuda = Device::cuda_if_available(0).map_err(|error| CandleDeviceError::Unavailable {
        preference: RuntimeDevicePreference::Auto,
        message: error.to_string(),
    })?;
    if !cuda.is_cpu() {
        return Ok(cuda);
    }

    let metal = Device::metal_if_available(0).map_err(|error| CandleDeviceError::Unavailable {
        preference: RuntimeDevicePreference::Auto,
        message: error.to_string(),
    })?;
    if !metal.is_cpu() {
        return Ok(metal);
    }

    Ok(Device::Cpu)
}
