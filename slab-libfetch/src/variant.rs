use crate::platform::Os;
use std::fmt;
use std::sync::OnceLock;

/// GPU/compute backend variant for an artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Variant {
    Cpu,
    Cuda,
    Vulkan,
    Metal,
}

impl Variant {
    /// Detect the best available compute backend for the given OS.
    ///
    /// Detection order:
    /// - macOS  → Metal
    /// - Windows/Linux → CUDA (if `nvcc`, `nvidia-smi`, or `CUDA_PATH` found)
    ///                 → Vulkan (if `vulkaninfo` or `VULKAN_SDK` found)
    ///                 → CPU fallback
    ///
    /// CUDA and Vulkan detection results are cached for the lifetime of the
    /// process to avoid repeated subprocess spawns.
    pub fn detect_best(os: &Os) -> Self {
        match os {
            Os::MacOS => Self::Metal,
            Os::Windows | Os::Linux => {
                if has_cuda() {
                    return Self::Cuda;
                }
                if has_vulkan() {
                    return Self::Vulkan;
                }
                Self::Cpu
            }
        }
    }
}

static CUDA_AVAILABLE: OnceLock<bool> = OnceLock::new();
static VULKAN_AVAILABLE: OnceLock<bool> = OnceLock::new();

fn has_cuda() -> bool {
    *CUDA_AVAILABLE.get_or_init(|| {
        std::env::var("CUDA_PATH").is_ok() || tool_exists("nvcc") || tool_exists("nvidia-smi")
    })
}

fn has_vulkan() -> bool {
    *VULKAN_AVAILABLE.get_or_init(|| {
        std::env::var("VULKAN_SDK").is_ok() || tool_exists("vulkaninfo")
    })
}

/// Returns `true` when `tool` can be found on PATH (any exit code is fine).
fn tool_exists(tool: &str) -> bool {
    std::process::Command::new(tool).arg("--version").output().is_ok()
}

impl fmt::Display for Variant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Variant::Cpu => write!(f, "cpu"),
            Variant::Cuda => write!(f, "cuda"),
            Variant::Vulkan => write!(f, "vulkan"),
            Variant::Metal => write!(f, "metal"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_variant_display() {
        assert_eq!(Variant::Cpu.to_string(), "cpu");
        assert_eq!(Variant::Cuda.to_string(), "cuda");
        assert_eq!(Variant::Vulkan.to_string(), "vulkan");
        assert_eq!(Variant::Metal.to_string(), "metal");
    }

    #[test]
    fn test_detect_best_macos_returns_metal() {
        assert_eq!(Variant::detect_best(&Os::MacOS), Variant::Metal);
    }

    #[test]
    fn test_detect_best_returns_cpu_or_gpu_variant() {
        // On Linux/Windows the result depends on the environment; we just
        // verify it returns one of the valid variants for that OS.
        let result = Variant::detect_best(&Os::Linux);
        assert!(
            matches!(result, Variant::Cpu | Variant::Cuda | Variant::Vulkan),
            "unexpected variant: {:?}",
            result
        );
        let result = Variant::detect_best(&Os::Windows);
        assert!(
            matches!(result, Variant::Cpu | Variant::Cuda | Variant::Vulkan),
            "unexpected variant: {:?}",
            result
        );
    }
}
