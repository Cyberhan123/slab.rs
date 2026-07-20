//! Chat-template helpers built on the llama.cpp FFI.
//!
//! **Important:** `llama_chat_apply_template` does **not** implement a full
//! jinja parser — it only supports a fixed list of builtin templates (ChatML,
//! Llama-2/3, Vicuna, …). For models with custom jinja templates (Qwen3.5,
//! GPT-OSS, …) the Rust minijinja renderer in `slab-app-core` remains the
//! primary path. These bindings exist for builtin-format models, builtin
//! detection, and tooling parity with `llama-cpp-rs-main`.

use std::ffi::CString;
use std::sync::Arc;

use crate::error::LlamaError;

/// A single `(role, content)` chat message passed to [`apply_chat_template`].
///
/// Owns its C strings so the resulting `llama_chat_message` stays valid for the
/// duration of the FFI call.
pub struct LlamaChatMessage {
    role: CString,
    content: CString,
}

impl LlamaChatMessage {
    /// Create a chat message from a role (`"system"` / `"user"` / `"assistant"`)
    /// and its content.
    ///
    /// # Errors
    /// Returns [`LlamaError::NullByteInString`] if `role` or `content` contain a NUL byte.
    pub fn new(role: &str, content: &str) -> Result<Self, LlamaError> {
        Ok(Self { role: CString::new(role)?, content: CString::new(content)? })
    }

    fn as_raw(&self) -> slab_llama_sys::llama_chat_message {
        slab_llama_sys::llama_chat_message {
            role: self.role.as_ptr(),
            content: self.content.as_ptr(),
        }
    }
}

/// Apply a chat template to `messages`, returning the formatted prompt string.
///
/// `tmpl` is the template source text; when `None`, llama.cpp selects its
/// default builtin template. Because the FFI only supports builtin templates,
/// callers with custom jinja should use the Rust renderer instead.
///
/// Two-pass sizing: llama.cpp reports the full formatted length even when the
/// buffer is too small, so we grow and retry until it fits.
pub(crate) fn apply_chat_template(
    lib: &Arc<slab_llama_sys::LlamaLib>,
    tmpl: Option<&str>,
    messages: &[LlamaChatMessage],
    add_ass: bool,
) -> Result<String, LlamaError> {
    let c_tmpl = match tmpl {
        Some(text) => Some(CString::new(text)?),
        None => None,
    };
    let tmpl_ptr = c_tmpl.as_ref().map(|c| c.as_ptr()).unwrap_or(std::ptr::null());
    let raw: Vec<slab_llama_sys::llama_chat_message> =
        messages.iter().map(LlamaChatMessage::as_raw).collect();

    let mut buf: Vec<u8> = vec![0u8; 256];
    loop {
        let n = unsafe {
            lib.llama_chat_apply_template(
                tmpl_ptr,
                raw.as_ptr(),
                raw.len(),
                add_ass,
                buf.as_mut_ptr() as *mut std::os::raw::c_char,
                buf.len() as i32,
            )
        };
        if n < 0 {
            return Err(LlamaError::ChatTemplateApplyFailed(n));
        }
        let n = n as usize;
        if n < buf.len() {
            buf.truncate(n);
            return String::from_utf8(buf).map_err(|e| LlamaError::from(e.utf8_error()));
        }
        // Buffer too small: llama.cpp reported the full required length. Grow and retry.
        buf.resize(n + 1, 0);
    }
}

/// List the names of builtin chat templates known to this llama.cpp build.
pub(crate) fn chat_builtin_templates(
    lib: &Arc<slab_llama_sys::LlamaLib>,
) -> Result<Vec<String>, LlamaError> {
    // Querying with a zero-capacity buffer yields the total builtin count.
    let count = unsafe { lib.llama_chat_builtin_templates(std::ptr::null_mut(), 0) };
    if count <= 0 {
        return Ok(Vec::new());
    }
    let mut ptrs: Vec<*const std::os::raw::c_char> = vec![std::ptr::null(); count as usize];
    let written = unsafe { lib.llama_chat_builtin_templates(ptrs.as_mut_ptr(), ptrs.len()) };
    let take = (written.max(0) as usize).min(ptrs.len());
    let mut out = Vec::with_capacity(take);
    for &p in &ptrs[..take] {
        if p.is_null() {
            continue;
        }
        if let Ok(name) = unsafe { std::ffi::CStr::from_ptr(p) }.to_str() {
            out.push(name.to_owned());
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_message_rejects_nul_bytes() {
        assert!(LlamaChatMessage::new("user\0", "hi").is_err());
        assert!(LlamaChatMessage::new("user", "hi\0").is_err());
    }

    #[test]
    fn chat_message_holds_valid_strings() {
        let message = LlamaChatMessage::new("user", "héllo 🦙").unwrap();
        let raw = message.as_raw();
        assert!(!raw.role.is_null());
        assert!(!raw.content.is_null());
    }
}
