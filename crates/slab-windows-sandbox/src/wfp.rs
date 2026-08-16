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

// The WFP provider + sublayer GUIDs are generated RANDOMLY per daemon instance (in
// `WfpEngine::register_package_block`), NOT fixed constants. Fixed keys collide across concurrent
// daemons (the elevated OS tests run 6 in parallel) and across a stray old daemon vs a freshly
// spawned one after a slab-server restart: the second daemon's `FwpmProviderAdd0` returns
// already-exists (the object is owned by the sibling's still-live session) and its filter that
// references that provider fails with FWP_E_WRONG_SESSION (0x8032000C). Random per-instance keys
// give each daemon its own objects — no cross-session conflict. The package SID the filter
// conditions on stays fingerprint-derived + stable, so the block still matches the spawned
// AppContainer child; only the provider/sublayer keys randomize.

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

    /// Register a provider + sublayer + V4/V6 outbound block filters scoped to `package_sid`.
    ///
    /// The provider/sublayer GUIDs are generated **randomly per daemon instance** (see the note on
    /// the removed module constants above), so each daemon owns its own objects. The filter adds +
    /// commit are the fail-closed gate. Every Fwpm step's return code is appended to a `trace`
    /// string surfaced in any error — the daemon's stdout/stderr are hidden behind
    /// `CREATE_NO_WINDOW`, so the trace (recorded in the unified sandbox audit log) is the only
    /// visibility into which step failed.
    pub(crate) fn register_package_block(
        &self,
        package_sid: PSID,
    ) -> Result<(), WindowsSandboxError> {
        // Per-instance random GUIDs. `WfpState::ensure_registered` calls this at most once per
        // daemon, so this is one provider/sublayer pair per daemon lifetime. Random (not fixed, not
        // fingerprint-derived) keys avoid cross-session collisions: a fixed key collides with
        // sibling daemons (the parallel OS tests) and with a stray old daemon after a slab-server
        // restart, surfacing as FWP_E_ALREADY_EXISTS on the add and FWP_E_WRONG_SESSION (0x8032000C)
        // on the filter. The package SID the filter conditions on stays fingerprint-derived +
        // stable, so the block still matches the spawned AppContainer child.
        let provider_guid = GUID::from_u128(uuid::Uuid::new_v4().as_u128());
        let sublayer_guid = GUID::from_u128(uuid::Uuid::new_v4().as_u128());

        // WFP requires a non-null `displayData.name` on every add, else FWP_E_NULL_DISPLAY_NAME
        // (0x80320023). The string is copied by the engine, so the Vec only needs to outlive the calls.
        let name = wide("slab-sandbox network block");
        let name_ptr = name.as_ptr() as *mut u16;
        let display = FWPM_DISPLAY_DATA0 { name: name_ptr, description: std::ptr::null_mut() };

        let mut trace = String::new();

        // SAFETY: all calls operate on our own engine handle with valid pointers.
        unsafe {
            // Provider + sublayer (this session's own, random keys) — untransactional auto-commit.
            // With unique keys there is no already-exists to tolerate; a non-zero here is unexpected,
            // but we record it into the trace and let the fail-closed filter add below surface it.
            let provider = FWPM_PROVIDER0 {
                providerKey: provider_guid,
                displayData: display,
                ..Default::default()
            };
            let rc = FwpmProviderAdd0(self.0, &provider, std::ptr::null_mut());
            trace.push_str(&format!("provider_add=0x{rc:x}; "));
            if rc != 0 {
                tracing::warn!(code = rc, "FwpmProviderAdd0 non-zero");
            }

            let sublayer = FWPM_SUBLAYER0 {
                subLayerKey: sublayer_guid,
                providerKey: &provider_guid as *const GUID as *mut GUID,
                weight: 0x4000,
                displayData: display,
                ..Default::default()
            };
            let rc = FwpmSubLayerAdd0(self.0, &sublayer, std::ptr::null_mut());
            trace.push_str(&format!("sublayer_add=0x{rc:x}; "));
            if rc != 0 {
                tracing::warn!(code = rc, "FwpmSubLayerAdd0 non-zero");
            }

            // Filters in their own transaction; both adds + commit must succeed (fail-closed).
            let rc = FwpmTransactionBegin0(self.0, 0);
            trace.push_str(&format!("txn_begin=0x{rc:x}; "));
            if rc != 0 {
                return Err(self.abort("FwpmTransactionBegin0", rc, &trace));
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
            let provider_key_ptr = &provider_guid as *const GUID as *mut GUID;

            let mut id: u64 = 0;
            let v4 = build_block_filter(
                FWPM_LAYER_ALE_AUTH_CONNECT_V4,
                sublayer_guid,
                provider_key_ptr,
                name_ptr,
                &mut condition,
            );
            let rc = FwpmFilterAdd0(self.0, &v4, std::ptr::null_mut(), &mut id);
            trace.push_str(&format!("filter_v4_add=0x{rc:x}; "));
            if rc != 0 {
                return Err(self.abort("FwpmFilterAdd0(V4)", rc, &trace));
            }

            let v6 = build_block_filter(
                FWPM_LAYER_ALE_AUTH_CONNECT_V6,
                sublayer_guid,
                provider_key_ptr,
                name_ptr,
                &mut condition,
            );
            let rc = FwpmFilterAdd0(self.0, &v6, std::ptr::null_mut(), &mut id);
            trace.push_str(&format!("filter_v6_add=0x{rc:x}; "));
            if rc != 0 {
                return Err(self.abort("FwpmFilterAdd0(V6)", rc, &trace));
            }

            let rc = FwpmTransactionCommit0(self.0);
            trace.push_str(&format!("commit=0x{rc:x}; "));
            if rc != 0 {
                return Err(self.abort("FwpmTransactionCommit0", rc, &trace));
            }
        }
        Ok(())
    }

    /// Abort the in-flight transaction and format an error including the per-step `trace`.
    fn abort(&self, ctx: &str, code: u32, trace: &str) -> WindowsSandboxError {
        // SAFETY: aborting a transaction on our own engine handle.
        unsafe {
            FwpmTransactionAbort0(self.0);
        }
        WindowsSandboxError::WindowsApi(format!(
            "{ctx} failed: code 0x{code:x} ({code}); trace: {trace}"
        ))
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

    /// Dummy sublayer GUID for the pure `build_block_filter` unit tests (the real code generates a
    /// random per-instance GUID; see `WfpEngine::register_package_block`).
    const SUBLAYER_GUID: GUID = GUID::from_u128(0x9e4b2d3f_5c8e_4b12_af70_223344556677);

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
