use anyhow::Result;

use super::payload::RuntimeVariant;

#[cfg(windows)]
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_ERROR_NOT_FOUND, IDXGIFactory1,
};

pub fn detect_best_variant() -> Result<RuntimeVariant> {
    #[cfg(windows)]
    {
        detect_best_variant_windows()
    }

    #[cfg(not(windows))]
    {
        Ok(RuntimeVariant::Base)
    }
}

#[cfg(windows)]
fn detect_best_variant_windows() -> Result<RuntimeVariant> {
    unsafe {
        let factory: IDXGIFactory1 = CreateDXGIFactory1()?;
        let mut found_amd = false;

        for index in 0.. {
            let adapter = match factory.EnumAdapters1(index) {
                Ok(adapter) => adapter,
                Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => break,
                Err(error) => return Err(error.into()),
            };

            let desc = adapter.GetDesc1()?;
            if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
                continue;
            }

            match desc.VendorId {
                0x10DE => return Ok(RuntimeVariant::Cuda),
                0x1002 | 0x1022 => found_amd = true,
                _ => {
                    let name = adapter_name(&desc.Description).to_ascii_lowercase();
                    if name.contains("nvidia") {
                        return Ok(RuntimeVariant::Cuda);
                    }
                    if name.contains("advanced micro devices")
                        || name.contains(" amd")
                        || name.starts_with("amd")
                        || name.contains("radeon")
                    {
                        found_amd = true;
                    }
                }
            }
        }

        Ok(if found_amd { RuntimeVariant::Hip } else { RuntimeVariant::Base })
    }
}

#[cfg(windows)]
fn adapter_name(buffer: &[u16]) -> String {
    let len = buffer.iter().position(|value| *value == 0).unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..len])
}
