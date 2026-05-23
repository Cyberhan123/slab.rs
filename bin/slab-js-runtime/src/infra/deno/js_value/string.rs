use deno_core::v8::{self, WriteFlags};
use serde::Deserialize;

use super::V8Value;

/// A Deserializable javascript UTF-16 string, that can be stored and used later
/// Must live as long as the runtime it was birthed from
#[derive(Eq, Hash, PartialEq, Debug, Clone)]
pub struct String(V8Value<StringTypeChecker>);
impl_v8!(String, StringTypeChecker);
impl_checker!(StringTypeChecker, String, is_string, |e| {
    crate::Error::JsonDecode(format!("Expected a string, found `{e}`"))
});

impl String {
    /// Converts the string to a rust string
    /// Potentially lossy, if the string contains orphan UTF-16 surrogates
    pub fn to_string_lossy(&self, runtime: &mut crate::Runtime) -> std::string::String {
        let isolate = runtime.deno_runtime().v8_isolate();
        let pinned_scope = std::pin::pin!(v8::HandleScope::new(isolate));
        let scope = pinned_scope.init();
        self.to_rust_string_lossy(&scope)
    }

    /// Converts the string to a rust string
    /// If the string contains orphan UTF-16 surrogates, it may return None
    /// In that case, you can use `to_string_lossy` to get a lossy conversion
    pub fn to_string(&self, runtime: &mut crate::Runtime) -> Option<std::string::String> {
        let bytes = self.to_utf8_bytes(runtime);
        std::string::String::from_utf8(bytes).ok()
    }

    /// Converts the string to a UTF-8 character buffer in the form of a `Vec<u8>`
    /// Excludes the null terminator
    pub fn to_utf8_bytes(&self, runtime: &mut crate::Runtime) -> Vec<u8> {
        let isolate = runtime.deno_runtime().v8_isolate();
        let pinned_scope = std::pin::pin!(v8::HandleScope::new(isolate));
        let scope = pinned_scope.init();
        self.to_utf8_buffer(&scope)
    }

    /// Converts the string to a UTF-16 character buffer in the form of a `Vec<u16>`
    /// Excludes the null terminator
    pub fn to_utf16_bytes(&self, runtime: &mut crate::Runtime) -> Vec<u16> {
        let isolate = runtime.deno_runtime().v8_isolate();
        let pinned_scope = std::pin::pin!(v8::HandleScope::new(isolate));
        let scope = pinned_scope.init();
        self.to_utf16_buffer(&scope)
    }

    pub(crate) fn to_rust_string_lossy<C>(
        &self,
        scope: &v8::PinnedRef<'_, v8::HandleScope<'_, C>>,
    ) -> std::string::String {
        let local = self.0.as_local(scope);
        // SAFETY: v8::String::to_rust_string_lossy requires &Isolate. PinnedRef<HandleScope> can be transmuted to &Isolate.
        let isolate = unsafe { &*std::ptr::from_ref(scope).cast::<v8::Isolate>() };
        local.to_rust_string_lossy(isolate)
    }

    pub(crate) fn to_utf16_buffer<C>(
        &self,
        scope: &v8::PinnedRef<'_, v8::HandleScope<'_, C>>,
    ) -> Vec<u16> {
        let local = self.0.as_local(scope);
        let u16_len = local.length();
        let mut buffer = vec![0; u16_len];

        // SAFETY: v8 methods may require &Isolate. PinnedRef<HandleScope> can be transmuted to &Isolate.
        let isolate = unsafe { &*std::ptr::from_ref(scope).cast::<v8::Isolate>() };
        local.write_v2(isolate, 0, &mut buffer, WriteFlags::empty());
        buffer
    }

    pub(crate) fn to_utf8_buffer<C>(
        &self,
        scope: &v8::PinnedRef<'_, v8::HandleScope<'_, C>>,
    ) -> Vec<u8> {
        let local = self.0.as_local(scope);
        // SAFETY: v8 methods may require &Isolate. PinnedRef<HandleScope> can be transmuted to &Isolate.
        let isolate = unsafe { &*std::ptr::from_ref(scope).cast::<v8::Isolate>() };
        let u8_len = local.utf8_length(isolate);
        let mut buffer = vec![0; u8_len];

        local.write_utf8_v2(isolate, &mut buffer, WriteFlags::empty(), None);
        buffer
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{Module, Runtime, RuntimeOptions};

    #[test]
    fn test_string() {
        let module = Module::new(
            "test.js",
            "
            // Valid UTF-8
            export const good = 'Hello, World!';

            // Invalid UTF-8, valid UTF-16
            export const bad = '\\ud83d\\ude00';
        ",
        );

        let mut runtime = Runtime::new(RuntimeOptions::default()).unwrap();
        let handle = runtime.load_module(&module).unwrap();

        let f: String = runtime.get_value(Some(&handle), "good").unwrap();
        let value = f.to_string_lossy(&mut runtime);
        assert_eq!(value, "Hello, World!");

        let f: String = runtime.get_value(Some(&handle), "good").unwrap();
        let value = f.to_string(&mut runtime).unwrap();
        assert_eq!(value, "Hello, World!");

        let f: String = runtime.get_value(Some(&handle), "bad").unwrap();
        let value = f.to_utf16_bytes(&mut runtime);
        assert_eq!(value, vec![0xd83d, 0xde00]);
    }
}
