//! The orchestrator side of the elevated round-trip: write a signed payload, launch the helper
//! elevated via `ShellExecuteExW("runas")`, wait for it, and read back the signed result. The
//! `Elevator` trait abstracts the launch+wait so the fail-closed paths (decline, timeout, tag
//! mismatch) are unit-testable without a real UAC prompt.
//!
//! Fail-closed matrix: UAC decline ⇒ `ElevationDeclined`; 60s timeout ⇒ `ElevationTimeout`;
//! non-zero helper exit ⇒ `HelperExit`; result tag mismatch ⇒ `Ipc(Hmac)`; nonce mismatch ⇒
//! `ElevationFailed`. None of these leave stale rules behind.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;

use crate::error::WindowsSandboxError;
use crate::ipc::{self, ElevationPayload, HelperResult};

/// Why an elevated launch did not produce a usable helper run.
#[derive(Debug, Error)]
pub enum HelperLaunchError {
    #[error("user declined elevation")]
    Declined,
    #[error("elevation timed out")]
    Timeout,
    #[error("elevation launch failed: {0}")]
    Failed(String),
}

/// Abstracts "launch the helper elevated and wait for it to exit". Production: `ShellElevator`
/// (`ShellExecuteExW("runas")` + `WaitForSingleObject`); tests supply a stub.
pub trait Elevator: Send + Sync {
    /// Launch `helper_exe` with `--payload <payload_path>` elevated; return the helper exit code
    /// (0 = success). Distinguish decline/timeout via [`HelperLaunchError`].
    fn run(&self, helper_exe: &Path, payload_path: &Path) -> Result<i32, HelperLaunchError>;
}

/// Real UAC elevation via `ShellExecuteExW`. Default timeout 60s.
pub struct ShellElevator {
    timeout_ms: u32,
}

impl Default for ShellElevator {
    fn default() -> Self {
        Self { timeout_ms: 60_000 }
    }
}

impl ShellElevator {
    pub fn new(timeout_ms: u32) -> Self {
        Self { timeout_ms }
    }
}

/// Windows status constants (defined locally to avoid a feature-path rabbit hole).
const ERROR_CANCELLED: u32 = 1223;
const WAIT_OBJECT_0: u32 = 0;
const WAIT_TIMEOUT: u32 = 0x0000_0102;
const WAIT_FAILED: u32 = 0xffff_ffff;

impl Elevator for ShellElevator {
    fn run(&self, helper_exe: &Path, payload_path: &Path) -> Result<i32, HelperLaunchError> {
        use windows_sys::Win32::Foundation::GetLastError;
        use windows_sys::Win32::System::Threading::{
            GetExitCodeProcess, TerminateProcess, WaitForSingleObject,
        };
        use windows_sys::Win32::UI::Shell::{
            SEE_MASK_FLAG_NO_UI, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
        };
        use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

        let verb = wide("runas");
        let file = wide(&helper_exe.to_string_lossy());
        let params = wide(&format!("--payload \"{}\"", payload_path.to_string_lossy()));

        // SAFETY: zeroed struct then filled; wide strings own their NUL-terminated buffers for
        // the duration of the call. hProcess is owned below and closed before return.
        let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_FLAG_NO_UI;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = params.as_ptr();
        info.nShow = SW_HIDE;

        let ok = unsafe { ShellExecuteExW(&mut info) };
        if ok == 0 {
            let last = unsafe { GetLastError() };
            if last == ERROR_CANCELLED {
                return Err(HelperLaunchError::Declined);
            }
            return Err(HelperLaunchError::Failed(format!(
                "ShellExecuteExW failed (last error {last})"
            )));
        }

        let process = info.hProcess;
        if process.is_null() {
            return Err(HelperLaunchError::Failed(
                "ShellExecuteExW returned no process handle".into(),
            ));
        }

        // Wait for the helper to exit (it writes the result file before exiting).
        let waited = unsafe { WaitForSingleObject(process, self.timeout_ms) };
        let exit_code = match waited {
            WAIT_OBJECT_0 => {
                let mut code: u32 = 0;
                let got = unsafe { GetExitCodeProcess(process, &mut code) };
                if got == 0 {
                    let err = HelperLaunchError::Failed(format!(
                        "GetExitCodeProcess failed: {}",
                        std::io::Error::last_os_error()
                    ));
                    unsafe {
                        windows_sys::Win32::Foundation::CloseHandle(process);
                    }
                    return Err(err);
                }
                code as i32
            }
            WAIT_TIMEOUT => {
                // Best-effort kill, then fail-closed.
                unsafe {
                    TerminateProcess(process, 1);
                    windows_sys::Win32::Foundation::CloseHandle(process);
                }
                return Err(HelperLaunchError::Timeout);
            }
            _ => {
                let last = if waited == WAIT_FAILED { unsafe { GetLastError() } } else { waited };
                unsafe {
                    windows_sys::Win32::Foundation::CloseHandle(process);
                }
                return Err(HelperLaunchError::Failed(format!(
                    "WaitForSingleObject returned {last:#x}"
                )));
            }
        };

        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(process);
        }
        Ok(exit_code)
    }
}

/// Run a full signed round-trip: write payload, elevate, read+verify result.
pub fn elevate(
    payload: ElevationPayload,
    payload_path: &Path,
    key: &[u8],
    helper_exe: &Path,
    elevator: &(impl Elevator + ?Sized),
) -> Result<HelperResult, WindowsSandboxError> {
    ipc::write_signed_payload(payload_path, &payload, key)?;
    let exit = elevator.run(helper_exe, payload_path).map_err(|e| match e {
        HelperLaunchError::Declined => WindowsSandboxError::ElevationDeclined,
        HelperLaunchError::Timeout => WindowsSandboxError::ElevationTimeout,
        HelperLaunchError::Failed(m) => WindowsSandboxError::ElevationFailed(m),
    })?;
    if exit != 0 {
        return Err(WindowsSandboxError::HelperExit(exit));
    }
    let result = ipc::read_signed_result(&payload.result_path, key)?;
    if result.nonce != payload.nonce {
        return Err(WindowsSandboxError::ElevationFailed("nonce mismatch".into()));
    }
    Ok(result)
}

/// Orchestrator-side facade bundling the paths + key needed to drive the helper.
pub struct ElevatedHelper {
    pub helper_exe: PathBuf,
    pub key_path: PathBuf,
    pub ipc_dir: PathBuf,
    pub marker_path: PathBuf,
}

impl ElevatedHelper {
    pub fn new(
        helper_exe: PathBuf,
        key_path: PathBuf,
        ipc_dir: PathBuf,
        marker_path: PathBuf,
    ) -> Self {
        Self { helper_exe, key_path, ipc_dir, marker_path }
    }

    /// One-shot Provision round-trip (the S2a smoke path; in S2b the helper applies real ACLs).
    pub fn provision_once(
        &self,
        elevator: &dyn Elevator,
    ) -> Result<HelperResult, WindowsSandboxError> {
        let key = crate::creds::load_or_create_key(&self.key_path)?;
        let fingerprint = crate::creds::key_fingerprint(&key);
        let nonce = uuid::Uuid::new_v4().simple().to_string();
        let now =
            SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
        let result_path = self.ipc_dir.join(format!("result-{nonce}.json"));
        let payload_path = self.ipc_dir.join(format!("payload-{nonce}.json"));
        let payload = ElevationPayload::new_provision(
            &nonce,
            now,
            fingerprint,
            result_path,
            self.marker_path.clone(),
        );
        elevate(payload, &payload_path, &key, &self.helper_exe, elevator)
    }
}

/// Encode a string as a NUL-terminated UTF-16 buffer (for PCWSTR fields).
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::HelperResult;

    /// A stub elevator that simulates a helper run without any UAC.
    enum Stub {
        Decline,
        Timeout,
        Exit(i32),
        /// "Run" the helper: write a valid signed result, return exit 0.
        WriteResult(HelperResult),
        /// "Run" the helper but write a result signed with the WRONG key.
        WriteTampered(HelperResult),
    }

    impl Elevator for Stub {
        fn run(&self, _helper_exe: &Path, payload_path: &Path) -> Result<i32, HelperLaunchError> {
            let payload_bytes = std::fs::read(payload_path)
                .map_err(|e| HelperLaunchError::Failed(e.to_string()))?;
            let framed: crate::ipc::SignedPayload = serde_json::from_slice(&payload_bytes)
                .map_err(|e| HelperLaunchError::Failed(e.to_string()))?;
            match self {
                Stub::Decline => Err(HelperLaunchError::Declined),
                Stub::Timeout => Err(HelperLaunchError::Timeout),
                Stub::Exit(code) => Ok(*code),
                Stub::WriteResult(result) => {
                    crate::ipc::write_signed_result(
                        &framed.payload.result_path,
                        result,
                        &test_key(),
                    )
                    .map_err(|e| HelperLaunchError::Failed(e.to_string()))?;
                    Ok(0)
                }
                Stub::WriteTampered(result) => {
                    let wrong = b"wrong-key-32-bytes-__________________"[..32].to_vec();
                    crate::ipc::write_signed_result(&framed.payload.result_path, result, &wrong)
                        .map_err(|e| HelperLaunchError::Failed(e.to_string()))?;
                    Ok(0)
                }
            }
        }
    }

    fn test_key() -> Vec<u8> {
        b"ipc-test-key-32-bytes-_____________"[..32].to_vec()
    }

    fn ok_result(nonce: &str) -> HelperResult {
        HelperResult {
            schema: crate::SCHEMA_VERSION,
            nonce: nonce.into(),
            ok: true,
            setup_kind: crate::capability::WindowsSetupKind::JobObject,
            filesystem_isolation: crate::capability::FsIsolationStrength::Lexical,
            error: None,
            marker: None,
            spawn_pid: None,
        }
    }

    fn run_elevate(stub: Stub) -> Result<HelperResult, WindowsSandboxError> {
        let dir = tempfile::tempdir().unwrap();
        let nonce = "nonce-xyz";
        let result_path = dir.path().join("result.json");
        let payload_path = dir.path().join("payload.json");
        let payload = ElevationPayload::new_provision(
            nonce,
            0,
            crate::creds::key_fingerprint(&test_key()),
            result_path,
            dir.path().join("marker.json"),
        );
        elevate(payload, &payload_path, &test_key(), Path::new("helper.exe"), &stub)
    }

    #[test]
    fn decline_returns_err() {
        let err = run_elevate(Stub::Decline).unwrap_err();
        assert!(matches!(err, WindowsSandboxError::ElevationDeclined), "{err:?}");
    }

    #[test]
    fn timeout_returns_err() {
        let err = run_elevate(Stub::Timeout).unwrap_err();
        assert!(matches!(err, WindowsSandboxError::ElevationTimeout), "{err:?}");
    }

    #[test]
    fn nonzero_exit_returns_err() {
        let err = run_elevate(Stub::Exit(7)).unwrap_err();
        assert!(matches!(err, WindowsSandboxError::HelperExit(7)), "{err:?}");
    }

    #[test]
    fn success_returns_result() {
        let r = run_elevate(Stub::WriteResult(ok_result("nonce-xyz"))).unwrap();
        assert!(r.ok);
    }

    #[test]
    fn tag_mismatch_returns_err() {
        let err = run_elevate(Stub::WriteTampered(ok_result("nonce-xyz"))).unwrap_err();
        assert!(
            matches!(err, WindowsSandboxError::Ipc(crate::ipc::FileFramingError::Hmac)),
            "{err:?}"
        );
    }
}
