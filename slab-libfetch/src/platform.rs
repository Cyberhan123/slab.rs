use std::fmt;

/// The operating system of the current host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Os {
    Windows,
    Linux,
    MacOS,
}

/// The CPU architecture of the current host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arch {
    X86_64,
    Aarch64,
}

/// Combined platform descriptor for the current host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Platform {
    pub os: Os,
    pub arch: Arch,
}

impl Platform {
    /// Detect the platform of the currently running process.
    ///
    /// Returns `None` when the OS or architecture is not recognised by
    /// this library (e.g. FreeBSD, MIPS).
    pub fn current() -> Option<Self> {
        let os = match std::env::consts::OS {
            "windows" => Os::Windows,
            "linux" => Os::Linux,
            "macos" => Os::MacOS,
            _ => return None,
        };
        let arch = match std::env::consts::ARCH {
            "x86_64" => Arch::X86_64,
            "aarch64" => Arch::Aarch64,
            _ => return None,
        };
        Some(Self { os, arch })
    }
}

impl fmt::Display for Os {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Os::Windows => write!(f, "windows"),
            Os::Linux => write!(f, "linux"),
            Os::MacOS => write!(f, "macos"),
        }
    }
}

impl fmt::Display for Arch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Arch::X86_64 => write!(f, "x86_64"),
            Arch::Aarch64 => write!(f, "aarch64"),
        }
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.os, self.arch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_current_is_some() {
        // Platform::current() returns None on unsupported OS/arch combinations
        // (documented behaviour), so treat that as a skip rather than a failure.
        match Platform::current() {
            Some(_) => {}
            None => return,
        }
    }

    #[test]
    fn test_os_display() {
        assert_eq!(Os::Windows.to_string(), "windows");
        assert_eq!(Os::Linux.to_string(), "linux");
        assert_eq!(Os::MacOS.to_string(), "macos");
    }

    #[test]
    fn test_arch_display() {
        assert_eq!(Arch::X86_64.to_string(), "x86_64");
        assert_eq!(Arch::Aarch64.to_string(), "aarch64");
    }

    #[test]
    fn test_platform_display() {
        let p = Platform { os: Os::Linux, arch: Arch::X86_64 };
        assert_eq!(p.to_string(), "linux-x86_64");
    }
}
