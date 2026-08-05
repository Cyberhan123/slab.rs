//! Model quantization API (`llama_model_quantize`).
//!
//! Mirrors the modern `llama_model_quantize_params` surface, including the
//! **turbo quantization types** [`LlamaFtype::MostlyTq1_0`] / `MostlyTq2_0`
//! used by 1.58-bit / ternary models, plus [`LlamaFtype::MostlyQ2_0`]. The
//! Hadamard-rotation `attn_rot` "TurboQuant" (llama.cpp PR #21038) is toggled
//! at runtime via the `LLAMA_ATTN_ROT_DISABLE` env var (read at context init)
//! — see [`set_attn_rot_disabled`]. It is not an FFI symbol, so there is nothing
//! to bind here; whether a given prebuilt DLL honors it depends on the build.
//!
//! The backend must be initialised ([`crate::Llama::backend_init`], done by
//! [`crate::Llama::new`]) before calling [`crate::Llama::model_quantize`].

use std::ffi::CString;

use crate::Llama;
use crate::error::LlamaError;

/// A `ggml_type` value, wrapped so callers do not depend on raw integer codes.
///
/// `Default` is `F32`, which in `llama_model_quantize_params` means
/// "use the file type's default" for the override tensor types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GgmlType(pub i32);

impl GgmlType {
    /// Sentinel meaning "use the file type's default".
    pub const DEFAULT: GgmlType = GgmlType(0);
    pub const F32: GgmlType = GgmlType(0);
    pub const F16: GgmlType = GgmlType(1);
    pub const BF16: GgmlType = GgmlType(30);
    pub const Q4_0: GgmlType = GgmlType(2);
    pub const Q8_0: GgmlType = GgmlType(8);
}

/// GGUF file type / quantization format.
///
/// Covers the formats exposed by the vendored `llama.h` (upstream `v10159`),
/// including the turbo types `TQ1_0`/`TQ2_0` and `Q2_0`. [`LlamaFtype::Unknown`]
/// preserves any forward-compatible value the build did not name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlamaFtype {
    AllF32,
    MostlyF16,
    MostlyQ4_0,
    MostlyQ4_1,
    MostlyQ8_0,
    MostlyQ5_0,
    MostlyQ5_1,
    MostlyQ2K,
    MostlyQ3KS,
    MostlyQ3KM,
    MostlyQ3KL,
    MostlyQ4KS,
    MostlyQ4KM,
    MostlyQ5KS,
    MostlyQ5KM,
    MostlyQ6K,
    MostlyIq2Xxs,
    MostlyIq2Xs,
    MostlyQ2KS,
    MostlyIq3Xs,
    MostlyIq3Xxs,
    MostlyIq1S,
    MostlyIq4Nl,
    MostlyIq3S,
    MostlyIq3M,
    MostlyIq2S,
    MostlyIq2M,
    MostlyIq4Xs,
    MostlyIq1M,
    MostlyBf16,
    MostlyTq1_0,
    MostlyTq2_0,
    MostlyMxfp4Moe,
    MostlyNvfp4,
    MostlyQ1_0,
    MostlyQ2_0,
    Unknown(i32),
}

impl LlamaFtype {
    /// Parse the common lowercase names used by `llama-quantize` and configs.
    pub fn from_name(name: &str) -> Self {
        match name.trim().to_ascii_lowercase().trim_start_matches("mostly_") {
            "f32" | "all_f32" => Self::AllF32,
            "f16" => Self::MostlyF16,
            "bf16" => Self::MostlyBf16,
            "q4_0" => Self::MostlyQ4_0,
            "q4_1" => Self::MostlyQ4_1,
            "q5_0" => Self::MostlyQ5_0,
            "q5_1" => Self::MostlyQ5_1,
            "q8_0" => Self::MostlyQ8_0,
            "q2_k" => Self::MostlyQ2K,
            "q2_k_s" => Self::MostlyQ2KS,
            "q3_k_s" => Self::MostlyQ3KS,
            "q3_k_m" => Self::MostlyQ3KM,
            "q3_k_l" => Self::MostlyQ3KL,
            "q4_k_s" => Self::MostlyQ4KS,
            "q4_k_m" => Self::MostlyQ4KM,
            "q5_k_s" => Self::MostlyQ5KS,
            "q5_k_m" => Self::MostlyQ5KM,
            "q6_k" => Self::MostlyQ6K,
            "iq2_xxs" => Self::MostlyIq2Xxs,
            "iq2_xs" => Self::MostlyIq2Xs,
            "iq2_s" => Self::MostlyIq2S,
            "iq2_m" => Self::MostlyIq2M,
            "iq3_xs" => Self::MostlyIq3Xs,
            "iq3_xxs" => Self::MostlyIq3Xxs,
            "iq3_s" => Self::MostlyIq3S,
            "iq3_m" => Self::MostlyIq3M,
            "iq1_s" => Self::MostlyIq1S,
            "iq1_m" => Self::MostlyIq1M,
            "iq4_nl" => Self::MostlyIq4Nl,
            "iq4_xs" => Self::MostlyIq4Xs,
            "tq1_0" => Self::MostlyTq1_0,
            "tq2_0" => Self::MostlyTq2_0,
            "mxfp4_moe" => Self::MostlyMxfp4Moe,
            "nvfp4" => Self::MostlyNvfp4,
            "q1_0" => Self::MostlyQ1_0,
            "q2_0" => Self::MostlyQ2_0,
            _ => Self::Unknown(-1),
        }
    }

    /// The canonical lowercase name (matches `from_name`), or `None` for `Unknown`.
    pub fn name(&self) -> Option<&'static str> {
        match self {
            Self::AllF32 => Some("all_f32"),
            Self::MostlyF16 => Some("f16"),
            Self::MostlyBf16 => Some("bf16"),
            Self::MostlyQ4_0 => Some("q4_0"),
            Self::MostlyQ4_1 => Some("q4_1"),
            Self::MostlyQ5_0 => Some("q5_0"),
            Self::MostlyQ5_1 => Some("q5_1"),
            Self::MostlyQ8_0 => Some("q8_0"),
            Self::MostlyQ2K => Some("q2_k"),
            Self::MostlyQ2KS => Some("q2_k_s"),
            Self::MostlyQ3KS => Some("q3_k_s"),
            Self::MostlyQ3KM => Some("q3_k_m"),
            Self::MostlyQ3KL => Some("q3_k_l"),
            Self::MostlyQ4KS => Some("q4_k_s"),
            Self::MostlyQ4KM => Some("q4_k_m"),
            Self::MostlyQ5KS => Some("q5_k_s"),
            Self::MostlyQ5KM => Some("q5_k_m"),
            Self::MostlyQ6K => Some("q6_k"),
            Self::MostlyIq2Xxs => Some("iq2_xxs"),
            Self::MostlyIq2Xs => Some("iq2_xs"),
            Self::MostlyIq2S => Some("iq2_s"),
            Self::MostlyIq2M => Some("iq2_m"),
            Self::MostlyIq3Xs => Some("iq3_xs"),
            Self::MostlyIq3Xxs => Some("iq3_xxs"),
            Self::MostlyIq3S => Some("iq3_s"),
            Self::MostlyIq3M => Some("iq3_m"),
            Self::MostlyIq1S => Some("iq1_s"),
            Self::MostlyIq1M => Some("iq1_m"),
            Self::MostlyIq4Nl => Some("iq4_nl"),
            Self::MostlyIq4Xs => Some("iq4_xs"),
            Self::MostlyTq1_0 => Some("tq1_0"),
            Self::MostlyTq2_0 => Some("tq2_0"),
            Self::MostlyMxfp4Moe => Some("mxfp4_moe"),
            Self::MostlyNvfp4 => Some("nvfp4"),
            Self::MostlyQ1_0 => Some("q1_0"),
            Self::MostlyQ2_0 => Some("q2_0"),
            Self::Unknown(_) => None,
        }
    }

    pub(crate) fn to_raw(self) -> i32 {
        use slab_llama_sys::*;
        match self {
            Self::AllF32 => llama_ftype_LLAMA_FTYPE_ALL_F32,
            Self::MostlyF16 => llama_ftype_LLAMA_FTYPE_MOSTLY_F16,
            Self::MostlyQ4_0 => llama_ftype_LLAMA_FTYPE_MOSTLY_Q4_0,
            Self::MostlyQ4_1 => llama_ftype_LLAMA_FTYPE_MOSTLY_Q4_1,
            Self::MostlyQ8_0 => llama_ftype_LLAMA_FTYPE_MOSTLY_Q8_0,
            Self::MostlyQ5_0 => llama_ftype_LLAMA_FTYPE_MOSTLY_Q5_0,
            Self::MostlyQ5_1 => llama_ftype_LLAMA_FTYPE_MOSTLY_Q5_1,
            Self::MostlyQ2K => llama_ftype_LLAMA_FTYPE_MOSTLY_Q2_K,
            Self::MostlyQ3KS => llama_ftype_LLAMA_FTYPE_MOSTLY_Q3_K_S,
            Self::MostlyQ3KM => llama_ftype_LLAMA_FTYPE_MOSTLY_Q3_K_M,
            Self::MostlyQ3KL => llama_ftype_LLAMA_FTYPE_MOSTLY_Q3_K_L,
            Self::MostlyQ4KS => llama_ftype_LLAMA_FTYPE_MOSTLY_Q4_K_S,
            Self::MostlyQ4KM => llama_ftype_LLAMA_FTYPE_MOSTLY_Q4_K_M,
            Self::MostlyQ5KS => llama_ftype_LLAMA_FTYPE_MOSTLY_Q5_K_S,
            Self::MostlyQ5KM => llama_ftype_LLAMA_FTYPE_MOSTLY_Q5_K_M,
            Self::MostlyQ6K => llama_ftype_LLAMA_FTYPE_MOSTLY_Q6_K,
            Self::MostlyIq2Xxs => llama_ftype_LLAMA_FTYPE_MOSTLY_IQ2_XXS,
            Self::MostlyIq2Xs => llama_ftype_LLAMA_FTYPE_MOSTLY_IQ2_XS,
            Self::MostlyQ2KS => llama_ftype_LLAMA_FTYPE_MOSTLY_Q2_K_S,
            Self::MostlyIq3Xs => llama_ftype_LLAMA_FTYPE_MOSTLY_IQ3_XS,
            Self::MostlyIq3Xxs => llama_ftype_LLAMA_FTYPE_MOSTLY_IQ3_XXS,
            Self::MostlyIq1S => llama_ftype_LLAMA_FTYPE_MOSTLY_IQ1_S,
            Self::MostlyIq4Nl => llama_ftype_LLAMA_FTYPE_MOSTLY_IQ4_NL,
            Self::MostlyIq3S => llama_ftype_LLAMA_FTYPE_MOSTLY_IQ3_S,
            Self::MostlyIq3M => llama_ftype_LLAMA_FTYPE_MOSTLY_IQ3_M,
            Self::MostlyIq2S => llama_ftype_LLAMA_FTYPE_MOSTLY_IQ2_S,
            Self::MostlyIq2M => llama_ftype_LLAMA_FTYPE_MOSTLY_IQ2_M,
            Self::MostlyIq4Xs => llama_ftype_LLAMA_FTYPE_MOSTLY_IQ4_XS,
            Self::MostlyIq1M => llama_ftype_LLAMA_FTYPE_MOSTLY_IQ1_M,
            Self::MostlyBf16 => llama_ftype_LLAMA_FTYPE_MOSTLY_BF16,
            Self::MostlyTq1_0 => llama_ftype_LLAMA_FTYPE_MOSTLY_TQ1_0,
            Self::MostlyTq2_0 => llama_ftype_LLAMA_FTYPE_MOSTLY_TQ2_0,
            Self::MostlyMxfp4Moe => llama_ftype_LLAMA_FTYPE_MOSTLY_MXFP4_MOE,
            Self::MostlyNvfp4 => llama_ftype_LLAMA_FTYPE_MOSTLY_NVFP4,
            Self::MostlyQ1_0 => llama_ftype_LLAMA_FTYPE_MOSTLY_Q1_0,
            Self::MostlyQ2_0 => llama_ftype_LLAMA_FTYPE_MOSTLY_Q2_0,
            Self::Unknown(raw) => raw,
        }
    }

    /// Inverse of `to_raw`: map a raw `llama_ftype` int back to
    /// a named variant when it is known, otherwise [`LlamaFtype::Unknown`].
    ///
    /// Used by callers (e.g. the runtime RPC layer) that receive the file type
    /// as an opaque integer over the wire.
    pub fn from_raw(raw: i32) -> Self {
        KNOWN_FTYPES
            .iter()
            .copied()
            .find(|ftype| ftype.to_raw() == raw)
            .unwrap_or(Self::Unknown(raw))
    }
}

/// All named [`LlamaFtype`] variants. [`LlamaFtype::from_raw`] iterates this to
/// invert [`LlamaFtype::to_raw`] without duplicating the constant mapping.
const KNOWN_FTYPES: &[LlamaFtype] = &[
    LlamaFtype::AllF32,
    LlamaFtype::MostlyF16,
    LlamaFtype::MostlyQ4_0,
    LlamaFtype::MostlyQ4_1,
    LlamaFtype::MostlyQ8_0,
    LlamaFtype::MostlyQ5_0,
    LlamaFtype::MostlyQ5_1,
    LlamaFtype::MostlyQ2K,
    LlamaFtype::MostlyQ3KS,
    LlamaFtype::MostlyQ3KM,
    LlamaFtype::MostlyQ3KL,
    LlamaFtype::MostlyQ4KS,
    LlamaFtype::MostlyQ4KM,
    LlamaFtype::MostlyQ5KS,
    LlamaFtype::MostlyQ5KM,
    LlamaFtype::MostlyQ6K,
    LlamaFtype::MostlyIq2Xxs,
    LlamaFtype::MostlyIq2Xs,
    LlamaFtype::MostlyQ2KS,
    LlamaFtype::MostlyIq3Xs,
    LlamaFtype::MostlyIq3Xxs,
    LlamaFtype::MostlyIq1S,
    LlamaFtype::MostlyIq4Nl,
    LlamaFtype::MostlyIq3S,
    LlamaFtype::MostlyIq3M,
    LlamaFtype::MostlyIq2S,
    LlamaFtype::MostlyIq2M,
    LlamaFtype::MostlyIq4Xs,
    LlamaFtype::MostlyIq1M,
    LlamaFtype::MostlyBf16,
    LlamaFtype::MostlyTq1_0,
    LlamaFtype::MostlyTq2_0,
    LlamaFtype::MostlyMxfp4Moe,
    LlamaFtype::MostlyNvfp4,
    LlamaFtype::MostlyQ1_0,
    LlamaFtype::MostlyQ2_0,
];

/// Parameters for [`Llama::model_quantize`].
///
/// Mirrors `llama_model_quantize_params`. The advanced pointer fields
/// (`imatrix`, `kv_overrides`, `tt_overrides`, `prune_layers`) are not yet
/// exposed here; they default to NULL.
#[derive(Debug, Clone)]
pub struct LlamaQuantizeParams {
    /// Number of threads to use during quantization (0 = let llama.cpp decide).
    pub nthread: i32,
    /// Target quantization format.
    pub ftype: LlamaFtype,
    /// Override type for the `output` tensor (`GgmlType::DEFAULT` = use ftype).
    pub output_tensor_type: GgmlType,
    /// Override type for the `token_embd` tensor (`GgmlType::DEFAULT` = use ftype).
    pub token_embedding_type: GgmlType,
    /// Allow quantizing already-quantized tensors (re-quantization).
    pub allow_requantize: bool,
    /// Quantize the `output` tensor (usually leave `true`).
    pub quantize_output_tensor: bool,
    /// Only copy tensors instead of quantizing.
    pub only_copy: bool,
    /// Disable mix-and-match of quantization types when not specified.
    pub pure: bool,
    /// Keep the model split layout (do not merge/split).
    pub keep_split: bool,
    /// Do not write a file — only report what would happen.
    pub dry_run: bool,
}

impl Default for LlamaQuantizeParams {
    fn default() -> Self {
        // Mirrors `llama_model_quantize_default_params()` from llama.cpp so
        // callers can override only the fields they care about (typically
        // `ftype`). Hard-coded because the FFI default-params getter requires a
        // loaded library handle and these defaults are stable.
        Self {
            nthread: 0,
            ftype: LlamaFtype::MostlyQ4_0,
            output_tensor_type: GgmlType::DEFAULT,
            token_embedding_type: GgmlType::DEFAULT,
            allow_requantize: false,
            quantize_output_tensor: true,
            only_copy: false,
            pure: false,
            keep_split: false,
            dry_run: false,
        }
    }
}

impl LlamaQuantizeParams {
    pub(crate) fn to_raw(&self) -> slab_llama_sys::llama_model_quantize_params {
        slab_llama_sys::llama_model_quantize_params {
            nthread: self.nthread,
            ftype: self.ftype.to_raw(),
            output_tensor_type: self.output_tensor_type.0,
            token_embedding_type: self.token_embedding_type.0,
            allow_requantize: self.allow_requantize,
            quantize_output_tensor: self.quantize_output_tensor,
            only_copy: self.only_copy,
            pure_: self.pure,
            keep_split: self.keep_split,
            dry_run: self.dry_run,
            imatrix: std::ptr::null(),
            kv_overrides: std::ptr::null(),
            tt_overrides: std::ptr::null(),
            prune_layers: std::ptr::null(),
        }
    }
}

impl Llama {
    /// Quantize `input_path` (a GGUF model) into `output_path` using `params`.
    ///
    /// Returns the number of layers processed (non-zero indicates success).
    /// The backend must already be initialised (done by [`Llama::new`]).
    ///
    /// # Errors
    /// Returns [`LlamaError::NullByteInString`] for bad paths, or
    /// [`LlamaError::QuantizeFailed`] if llama.cpp reports failure.
    pub fn model_quantize(
        &self,
        input_path: &str,
        output_path: &str,
        params: &LlamaQuantizeParams,
    ) -> Result<u32, LlamaError> {
        let c_in = CString::new(input_path)?;
        let c_out = CString::new(output_path)?;
        let raw = params.to_raw();
        let count = unsafe {
            self.lib.llama_model_quantize(c_in.as_ptr(), c_out.as_ptr(), &raw as *const _)
        };
        if count == 0 { Err(LlamaError::QuantizeFailed) } else { Ok(count) }
    }
}

/// Env-var name read by llama.cpp at context creation to disable the Hadamard
/// attention rotation ("TurboQuant", PR #21038).
const ATTN_ROT_DISABLE_ENV: &str = "LLAMA_ATTN_ROT_DISABLE";

/// Toggle llama.cpp's TurboQuant attention rotation.
///
/// Sets the `LLAMA_ATTN_ROT_DISABLE` env var, which llama.cpp reads when a
/// `llama_context` is created. Must be called **before** any context is
/// initialised on the current process, and not concurrently with context
/// creation. Whether the rotation is actually applied depends on the prebuilt
/// `llama`/`mtmd` shared library having been compiled with PR #21038.
///
/// Pass `true` to opt out (disable TurboQuant); `false` restores the default
/// (rotation enabled).
//
// `std::env::set_var`/`remove_var` became `unsafe` in Rust 1.95+ because env
// mutation is not synchronised with concurrent readers. The caller is
// responsible for single-threaded setup here.
pub fn set_attn_rot_disabled(disabled: bool) {
    // SAFETY: caller guarantees no concurrent context creation / env reads.
    unsafe {
        if disabled {
            std::env::set_var(ATTN_ROT_DISABLE_ENV, "1");
        } else {
            std::env::remove_var(ATTN_ROT_DISABLE_ENV);
        }
    }
}

/// Whether TurboQuant attention rotation is currently disabled via the env var.
#[must_use]
pub fn attn_rot_disabled() -> bool {
    std::env::var(ATTN_ROT_DISABLE_ENV).ok().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ftype_round_trips_common_names() {
        for name in
            ["q4_k_m", "q4_K_M", " MOSTLY_Q4_K_M ", "q8_0", "bf16", "tq1_0", "tq2_0", "q6_k", "f16"]
        {
            let ftype = LlamaFtype::from_name(name);
            assert_ne!(ftype, LlamaFtype::Unknown(-1), "{name} should parse");
            let round = ftype.name().expect("parsed ftype has a name");
            let again = LlamaFtype::from_name(round);
            assert_eq!(ftype, again, "{name} round-trips via {round}");
        }
    }

    #[test]
    fn unknown_name_returns_unknown() {
        assert_eq!(LlamaFtype::from_name("nonsense"), LlamaFtype::Unknown(-1));
        assert!(LlamaFtype::Unknown(-1).name().is_none());
    }

    #[test]
    fn turbo_types_map_to_raw_codes() {
        assert_eq!(LlamaFtype::MostlyTq1_0.to_raw(), 36);
        assert_eq!(LlamaFtype::MostlyTq2_0.to_raw(), 37);
    }

    #[test]
    fn default_params_build_safely() {
        // Exercises the FFI default-params call without quantizing.
        let params =
            LlamaQuantizeParams { ftype: LlamaFtype::MostlyQ4KM, ..LlamaQuantizeParams::default() };
        let raw = params.to_raw();
        assert_eq!(raw.ftype, 15);
        assert!(raw.imatrix.is_null());
        assert!(raw.kv_overrides.is_null());
    }

    #[test]
    fn from_raw_round_trips_named_variants() {
        for variant in KNOWN_FTYPES {
            assert_eq!(LlamaFtype::from_raw(variant.to_raw()), *variant);
        }
        // Spot-check the wire-comment values from ggml/llama.proto.
        assert_eq!(LlamaFtype::from_raw(15), LlamaFtype::MostlyQ4KM);
        assert_eq!(LlamaFtype::from_raw(36), LlamaFtype::MostlyTq1_0);
        assert_eq!(LlamaFtype::from_raw(37), LlamaFtype::MostlyTq2_0);
    }

    #[test]
    fn from_raw_unknown_passthrough() {
        assert_eq!(LlamaFtype::from_raw(-1), LlamaFtype::Unknown(-1));
        assert_eq!(LlamaFtype::from_raw(9999), LlamaFtype::Unknown(9999));
    }

    #[test]
    fn q2_0_round_trips() {
        assert_eq!(LlamaFtype::from_name("q2_0"), LlamaFtype::MostlyQ2_0);
        assert_eq!(LlamaFtype::MostlyQ2_0.to_raw(), 41);
        assert_eq!(LlamaFtype::from_raw(41), LlamaFtype::MostlyQ2_0);
        assert_eq!(LlamaFtype::MostlyQ2_0.name(), Some("q2_0"));
    }

    #[test]
    fn attn_rot_toggle_round_trips() {
        // Capture and restore so the test is order-independent.
        let prior = std::env::var(ATTN_ROT_DISABLE_ENV).ok();
        set_attn_rot_disabled(true);
        assert!(attn_rot_disabled());
        set_attn_rot_disabled(false);
        assert!(!attn_rot_disabled());
        match prior {
            Some(value) => {
                // SAFETY: test-only, single-threaded.
                unsafe { std::env::set_var(ATTN_ROT_DISABLE_ENV, value) };
            }
            None => {
                // SAFETY: test-only, single-threaded.
                unsafe { std::env::remove_var(ATTN_ROT_DISABLE_ENV) };
            }
        }
    }
}
