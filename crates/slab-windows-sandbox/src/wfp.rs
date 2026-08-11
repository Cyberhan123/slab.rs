//! User-mode WFP (Windows Filtering Platform) network-block filters (S3).
//!
//! Registers a session-scoped provider + sublayer + two `FWP_ACTION_BLOCK` filters (IPv4 + IPv6
//! outbound ALE connect) keyed on the sandboxed child's AppContainer package SID via
//! `FWPM_CONDITION_ALE_PACKAGE_ID`. The session is opened with `FWPM_SESSION_FLAG_DYNAMIC`, so the
//! filters are auto-removed when the engine closes — i.e. when the elevated daemon exits — matching
//! the daemon's lifetime. No `FWPM_FILTER_FLAG_PERSISTENT` anywhere.
//!
//! The OS default WFP rule already blocks AppContainer processes that lack the `internetClient`
//! capability; this explicit filter is defense-in-depth AND gives us a filter WE registered, so the
//! honest `network_isolation = OsEnforced` report rests on something we control (rather than on an
//! opaque system default rule that group policy could alter).
//!
//! Fail-closed: any filter-add or commit error aborts the transaction and surfaces an `Err`, which
//! the daemon turns into a Provision failure ⇒ the shell stays blocked.

use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::NetworkManagement::WindowsFilteringPlatform::{
    FWP_ACTION_BLOCK, FWP_CONDITION_VALUE0, FWP_CONDITION_VALUE0_0, FWP_MATCH_EQUAL, FWP_SID,
    FWPM_ACTION0, FWPM_CONDITION_ALE_PACKAGE_ID, FWPM_DISPLAY_DATA0, FWPM_FILTER_CONDITION0,
    FWPM_FILTER0, FWPM_LAYER_ALE_AUTH_CONNECT_V4, FWPM_LAYER_ALE_AUTH_CONNECT_V6, FWPM_PROVIDER0,
    FWPM_SESSION_FLAG_DYNAMIC, FWPM_SESSION0, FWPM_SUBLAYER0, FwpmEngineClose0, FwpmEngineOpen0,
    FwpmFilterAdd0, FwpmProviderAdd0, FwpmSubLayerAdd0, FwpmTransactionAbort0,
    FwpmTransactionBegin0, FwpmTransactionCommit0,
};
use windows_sys::Win32::Security::{PSID, SID};
use windows_sys::Win32::System::Rpc::RPC_C_AUTHN_WINNT;
use windows_sys::core::GUID;

use crate::error::WindowsSandboxError;

/// Fixed identity of the slab-sandbox WFP provider (compile-time GUID). Providers/sublayers persist
/// across daemon restarts (unlike DYNAMIC-session filters), so re-registration tolerates "already
/// exists".
const PROVIDER_GUID: GUID = GUID::from_u128(0x8f3a1c2e_4b7d_4a01_9e6f_112233445566);
/// Fixed identity of the slab-sandbox WFP sublayer.
const SUBLAYER_GUID: GUID = GUID::from_u128(0x9e4b2d3f_5c8e_4b12_af70_223344556677);

/// An open WFP engine session. Dropping closes the engine, which (because the session is DYNAMIC)
/// removes every filter added through it.
pub(crate) struct WfpEngine(HANDLE);
// The engine handle is an opaque pointer; it lives behind `Arc<Mutex<Option<WfpEngine>>>` in the
// daemon and is accessed serialized, so manual `Send` is sound.
unsafe impl Send for WfpEngine {}

impl WfpEngine {
    /// Open a DYNAMIC engine session. Filters added via this session vanish on close (= daemon exit).
    pub(crate) fn open() -> Result<Self, WindowsSandboxError> {
        let session = FWPM_SESSION0 { flags: FWPM_SESSION_FLAG_DYNAMIC, ..Default::default() };
        let mut handle: HANDLE = std::ptr::null_mut();
        // SAFETY: null servername/authidentity; `session` is a valid pointer; `handle` is an out-param.
        let err = unsafe {
            FwpmEngineOpen0(
                std::ptr::null(),
                RPC_C_AUTHN_WINNT,
                std::ptr::null(),
                &session,
                &mut handle,
            )
        };
        if err != 0 {
            return Err(WindowsSandboxError::WindowsApi(format!(
                "FwpmEngineOpen0 failed: code {err}"
            )));
        }
        Ok(Self(handle))
    }

    /// Register provider + sublayer + V4/V6 outbound block filters scoped to `package_sid`.
    ///
    /// Provider/sublayer are idempotent (they persist across restarts; tolerate "already exists" or
    /// any non-zero there with a warning). The filter adds and the commit are the real fail-closed
    /// gate: if anything is genuinely broken the filter add fails and we abort.
    pub(crate) fn register_package_block(
        &self,
        package_sid: PSID,
    ) -> Result<(), WindowsSandboxError> {
        // WFP requires a non-null `displayData.name` on every provider/sublayer/filter add, else it
        // returns FWP_E_NULL_DISPLAY_NAME (0x80320023). The string is copied by the engine, so the
        // Vec only needs to outlive the calls below.
        let name = wide("slab-sandbox network block");
        let name_ptr = name.as_ptr() as *mut u16;
        let display = FWPM_DISPLAY_DATA0 { name: name_ptr, description: std::ptr::null_mut() };

        // SAFETY: all calls operate on our own engine handle with valid pointers.
        unsafe {
            // Provider + sublayer are PERSISTENT (survive daemon restarts) and idempotent. Add them
            // OUTSIDE the filter transaction and tolerate any non-zero: a re-provision legitimately
            // returns FWP_E_ALREADY_EXISTS, and — critically — an error returned by an add INSIDE an
            // explicit transaction aborts it, which would make the filter adds below fail with
            // FWP_E_NO_TXN_IN_PROGRESS (0x8032000C). Untransactional adds auto-commit individually,
            // so an already-exists here is harmless. The filter adds below are the fail-closed gate;
            // a genuinely-broken provider surfaces there as FWP_E_PROVIDER_NOT_FOUND.
            let provider = FWPM_PROVIDER0 {
                providerKey: PROVIDER_GUID,
                displayData: display,
                ..Default::default()
            };
            let err = FwpmProviderAdd0(self.0, &provider, std::ptr::null_mut());
            if err != 0 {
                tracing::warn!(
                    code = err,
                    "FwpmProviderAdd0 non-zero (likely already-exists; continuing)"
                );
            }

            let sublayer = FWPM_SUBLAYER0 {
                subLayerKey: SUBLAYER_GUID,
                providerKey: &PROVIDER_GUID as *const GUID as *mut GUID,
                weight: 0x4000,
                displayData: display,
                ..Default::default()
            };
            let err = FwpmSubLayerAdd0(self.0, &sublayer, std::ptr::null_mut());
            if err != 0 {
                tracing::warn!(
                    code = err,
                    "FwpmSubLayerAdd0 non-zero (likely already-exists; continuing)"
                );
            }

            // Fresh transaction for the two block filters only. The provider/sublayer already exist
            // (just added or persisted from a prior run), so the filters' providerKey/subLayerKey
            // resolve cleanly. If either add fails we abort and surface Err (fail-closed).
            let err = FwpmTransactionBegin0(self.0, 0);
            if err != 0 {
                return Err(self.abort("FwpmTransactionBegin0", err));
            }

            // One condition: match connections whose AppContainer package SID == package_sid.
            let mut condition = FWPM_FILTER_CONDITION0 {
                fieldKey: FWPM_CONDITION_ALE_PACKAGE_ID,
                matchType: FWP_MATCH_EQUAL,
                conditionValue: FWP_CONDITION_VALUE0 {
                    r#type: FWP_SID,
                    Anonymous: FWP_CONDITION_VALUE0_0 { sid: package_sid as *mut SID },
                },
            };
            let provider_key_ptr = &PROVIDER_GUID as *const GUID as *mut GUID;

            let mut id: u64 = 0;
            let v4 = build_block_filter(
                FWPM_LAYER_ALE_AUTH_CONNECT_V4,
                SUBLAYER_GUID,
                provider_key_ptr,
                name_ptr,
                &mut condition,
            );
            let err = FwpmFilterAdd0(self.0, &v4, std::ptr::null_mut(), &mut id);
            if err != 0 {
                return Err(self.abort("FwpmFilterAdd0(V4)", err));
            }

            let v6 = build_block_filter(
                FWPM_LAYER_ALE_AUTH_CONNECT_V6,
                SUBLAYER_GUID,
                provider_key_ptr,
                name_ptr,
                &mut condition,
            );
            let err = FwpmFilterAdd0(self.0, &v6, std::ptr::null_mut(), &mut id);
            if err != 0 {
                return Err(self.abort("FwpmFilterAdd0(V6)", err));
            }

            let err = FwpmTransactionCommit0(self.0);
            if err != 0 {
                return Err(self.abort("FwpmTransactionCommit0", err));
            }
        }
        Ok(())
    }

    /// Abort the in-flight transaction and format an error.
    fn abort(&self, ctx: &str, code: u32) -> WindowsSandboxError {
        // SAFETY: aborting a transaction on our own engine handle.
        unsafe {
            FwpmTransactionAbort0(self.0);
        }
        WindowsSandboxError::WindowsApi(format!("{ctx} failed: code {code}"))
    }
}

impl Drop for WfpEngine {
    fn drop(&mut self) {
        // SAFETY: closing our own engine handle; DYNAMIC session ⇒ filters auto-removed.
        unsafe { FwpmEngineClose0(self.0) };
    }
}

/// Build a block filter for `layer_key` (V4 or V6) carrying the package-SID condition. Pure: it does
/// NOT call `FwpmFilterAdd0`, so it is unit-testable without elevation.
fn build_block_filter(
    layer_key: GUID,
    sublayer_key: GUID,
    provider_key: *mut GUID,
    display_name: *mut u16,
    condition: &mut FWPM_FILTER_CONDITION0,
) -> FWPM_FILTER0 {
    FWPM_FILTER0 {
        layerKey: layer_key,
        subLayerKey: sublayer_key,
        providerKey: provider_key,
        displayData: FWPM_DISPLAY_DATA0 { name: display_name, description: std::ptr::null_mut() },
        flags: 0, // NO FWPM_FILTER_FLAG_PERSISTENT — session-scoped only
        numFilterConditions: 1,
        filterCondition: condition as *mut FWPM_FILTER_CONDITION0,
        action: FWPM_ACTION0 { r#type: FWP_ACTION_BLOCK, ..Default::default() },
        ..Default::default()
    }
}

/// Encode a string as a NUL-terminated UTF-16 buffer (for the WFP display-name PCWSTR fields).
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `windows_sys::core::GUID` has no `PartialEq`/`Debug` derives, so compare fields by hand.
    fn guid_eq(a: &GUID, b: &GUID) -> bool {
        a.data1 == b.data1 && a.data2 == b.data2 && a.data3 == b.data3 && a.data4 == b.data4
    }

    #[test]
    fn build_block_filter_sets_block_action_and_package_condition() {
        // Dummy condition storage; only its address is captured by the filter.
        let mut cond = FWPM_FILTER_CONDITION0 {
            fieldKey: FWPM_CONDITION_ALE_PACKAGE_ID,
            matchType: FWP_MATCH_EQUAL,
            conditionValue: FWP_CONDITION_VALUE0::default(),
        };
        let f = build_block_filter(
            FWPM_LAYER_ALE_AUTH_CONNECT_V4,
            SUBLAYER_GUID,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut cond,
        );
        assert_eq!(f.action.r#type, FWP_ACTION_BLOCK, "action must BLOCK");
        assert_eq!(f.flags, 0, "no PERSISTENT flag — session-scoped");
        assert_eq!(f.numFilterConditions, 1, "exactly one condition");
        assert!(guid_eq(&f.layerKey, &FWPM_LAYER_ALE_AUTH_CONNECT_V4), "V4 layer key");
        assert!(guid_eq(&f.subLayerKey, &SUBLAYER_GUID), "our sublayer");
        assert_eq!(
            f.filterCondition, &mut cond as *mut _,
            "filter references the provided condition"
        );
    }

    #[test]
    fn build_block_filter_v4_and_v6_use_distinct_layer_keys() {
        let mut cond = FWPM_FILTER_CONDITION0 {
            fieldKey: FWPM_CONDITION_ALE_PACKAGE_ID,
            matchType: FWP_MATCH_EQUAL,
            conditionValue: FWP_CONDITION_VALUE0::default(),
        };
        let v4 = build_block_filter(
            FWPM_LAYER_ALE_AUTH_CONNECT_V4,
            SUBLAYER_GUID,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut cond,
        );
        let v6 = build_block_filter(
            FWPM_LAYER_ALE_AUTH_CONNECT_V6,
            SUBLAYER_GUID,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut cond,
        );
        assert!(!guid_eq(&v4.layerKey, &v6.layerKey), "V4 and V6 layers must differ");
        assert_eq!(v4.action.r#type, FWP_ACTION_BLOCK);
        assert_eq!(v6.action.r#type, FWP_ACTION_BLOCK);
    }
}
