//! This module provides a way to store and use javascript values, functions, and promises
//! The are a deserialized version of the `v8::Value`
//!
//! [Function] and [Promise] are both specializations of [Value] providing deserialize-time type checking
//! and additional utility functions for interacting with the runtime
use deno_core::{serde_v8::GlobalValue, v8};
use serde::Deserialize;

/// A macro to implement the common functions for [Function], [Promise], and [Value]
macro_rules! impl_v8 {
    ($name:ident$(<$generic:ident>)?, $checker:ident $(,)?) => {
        impl $(<$generic>)? $name $(<$generic>)? where
        $( $generic: serde::de::DeserializeOwned, )? {
            /// Consume this struct and return the underlying `V8Value`
            #[allow(dead_code)]
            pub(crate) fn into_inner(self) -> V8Value<$checker> {
                self.0
            }

            /// Returns the underlying [`crate::deno_core::v8::Global`]
            /// This is useful if you want to pass the value to a [`crate::deno_core::JsRuntime`] function directly
            #[must_use]
            pub fn into_v8(self) -> v8::Global<v8::Value> {
                self.0 .0
            }

            /// Returns a reference to the underlying [`crate::deno_core::v8::Global`]
            /// This is useful if you want to pass the value to a [`crate::deno_core::JsRuntime`] function directly
            #[must_use]
            pub fn as_v8(&self) -> &v8::Global<v8::Value> {
                &self.0 .0
            }

            /// Creates a new instance of this struct from a global value
            ///
            /// # Errors
            /// Will return an error if the value is the wrong type
            /// For `Value`, this check cannot fail
            pub fn try_from_v8<'a, H>(
                scope: &mut v8::PinScope<'a, '_>,
                value: v8::Global<H>,
            ) -> Result<Self, crate::Error>
            where
                v8::Local<'a, v8::Value>: From<v8::Local<'a, H>>,
            {
                let local: v8::Local<v8::Value> = v8::Local::new(scope, value).into();
                // Get isolate reference from the scope using Deref
                let isolate: &v8::Isolate = scope;
                v8::Global::new(isolate, local).try_into()
            }

            /// Creates a new instance of this struct from a global value
            /// Makes no attempt to check the type of the value
            /// This can result in a panic if the value is not of the correct type
            ///
            /// # Safety
            /// This function is unsafe because it does not check the type of the value
            /// If the value is not of the correct type, a panic will occur
            /// It is recommended to use [`Self::try_from_v8`] instead
            #[must_use]
            pub unsafe fn from_v8_unchecked(value: v8::Global<v8::Value>) -> Self {
                let inner = V8Value::<$checker>(value, std::marker::PhantomData);
                Self(inner $(, std::marker::PhantomData::<$generic>)?)
            }
        }
        impl<'de$(, $generic)?> serde::Deserialize<'de> for $name $(<$generic>)?
        $(where $generic: serde::de::DeserializeOwned,)?
        {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let inner = V8Value::<$checker>::deserialize(deserializer)?;
                Ok(Self(inner $(, std::marker::PhantomData::<$generic>)?))
            }
        }

        #[allow(clippy::from_over_into)]
        impl $(<$generic>)? Into<v8::Global<v8::Value>> for $name $(<$generic>)? $(where $generic: serde::de::DeserializeOwned)? {
            fn into(self) -> v8::Global<v8::Value> {
                self.0 .0
            }
        }

        impl $(<$generic>)? TryFrom<v8::Global<v8::Value>> for $name $(<$generic>)? $(where $generic: serde::de::DeserializeOwned)? {
            type Error = crate::Error;
            fn try_from(value: v8::Global<v8::Value>) -> Result<Self, Self::Error> {
                <$checker as $crate::js_value::V8TypeChecker>::validate(value.clone())?;
                let inner = V8Value::<$checker>(value, std::marker::PhantomData);
                Ok(Self(inner $(, std::marker::PhantomData::<$generic>)?))
            }
        }
    };
}

/// A macro to implement type checkers for [Function], [Promise], and [Value]
macro_rules! impl_checker {
    ($name:ident, $v8_name:ident, $checker_fn:ident, |$err_ty:ident| $err:block) => {
        #[doc = "Implementations of `V8TypeChecker`"]
        #[doc = concat!("Guards for `v8::", stringify!($v8_name), "` values")]
        #[derive(Eq, Hash, PartialEq, Debug, Clone, Deserialize)]
        pub(crate) struct $name;
        impl $crate::js_value::V8TypeChecker for $name {
            type Output = v8::$v8_name;
            fn validate(value: v8::Global<v8::Value>) -> Result<(), crate::Error> {
                let raw: &v8::Value = unsafe { v8::Handle::get_unchecked(&value) };
                if raw.$checker_fn() {
                    Ok(())
                } else {
                    let $err_ty = raw.type_repr().to_string();
                    Err($err)
                }
            }
        }
    };

    ($name:ident, $v8_name:ident) => {
        #[doc = "Implementation of `V8TypeChecker`"]
        #[doc = concat!("Guards for `v8::", stringify!($v8_name), "` values")]
        #[derive(Eq, Hash, PartialEq, Debug, Clone, Deserialize)]
        pub(crate) struct $name;
        impl V8TypeChecker for $name {
            type Output = v8::$v8_name;
            fn validate(_: v8::Global<v8::Value>) -> Result<(), crate::Error> {
                Ok(())
            }
        }
    };
}

/// A trait that is used to check if a `v8::Value` is of a certain type
/// Will cause a panic if validate is insufficient to verify that the
/// given value is of type `T::Output`
pub(crate) trait V8TypeChecker {
    /// The v8 type that this checker guards for
    type Output;

    /// Checks if a `v8::Value` is of the output type
    /// If the value is not of the output type, an error is returned
    ///
    /// Note: If the guard is not sufficient to verify the type, a panic will occur
    /// when this checker is used
    fn validate(value: v8::Global<v8::Value>) -> Result<(), crate::Error>;
}

// For values
impl_checker!(DefaultTypeChecker, Value);

/// The core struct behind the [Function], [Promise], and [Value] types
/// Should probably not be user-facing
/// TODO: Safer API for this so we can make it public eventually
///
/// A Deserializable javascript object, that can be stored and used later
/// Must live as long as the runtime it was birthed from
#[derive(Eq, Hash, PartialEq, Debug, Clone)]
pub(crate) struct V8Value<V8TypeChecker>(
    v8::Global<v8::Value>,
    std::marker::PhantomData<V8TypeChecker>,
);

impl<T: V8TypeChecker> V8Value<T> {
    /// Returns the underlying global as a local in the type configured by the type checker
    pub(crate) fn as_local<'a, 'i, C>(
        &self,
        scope: &v8::PinnedRef<'a, v8::HandleScope<'i, C>>,
    ) -> v8::Local<'a, T::Output>
    where
        v8::Local<'a, T::Output>: TryFrom<v8::Local<'a, v8::Value>>,
    {
        // SAFETY: v8::Local::new requires HandleScope<'_, ()> but works with any context type.
        // The context parameter is a phantom type marker and doesn't affect memory layout.
        // We transmute the scope reference to the required type for the API call.
        let scope_ref = unsafe {
            &*std::ptr::from_ref(scope).cast::<v8::PinnedRef<'a, v8::HandleScope<'i, ()>>>()
        };
        let local = v8::Local::<v8::Value>::new(scope_ref, &self.0);
        v8::Local::<'a, T::Output>::try_from(local)
            .ok()
            .expect("Failed to convert V8Value: Invalid V8TypeChecker!")
    }

    /// Returns the underlying global in the type configured by the type checker
    /// Note: This performs an unchecked cast - the type checker should have validated the type
    pub(crate) fn as_global(&self, _isolate: &v8::Isolate) -> v8::Global<T::Output>
    where
        T::Output: 'static,
    {
        // SAFETY: The type checker should have validated that self.0 is of the correct type
        // We use transmute to convert v8::Global<v8::Value> to v8::Global<T::Output>
        // This is safe because v8::Global is just a pointer wrapper
        unsafe { std::mem::transmute(self.0.clone()) }
    }
}

impl<'de, T: V8TypeChecker> serde::Deserialize<'de> for V8Value<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = GlobalValue::deserialize(deserializer)?;
        T::validate(value.v8_value.clone()).map_err(serde::de::Error::custom)?;
        Ok(Self(value.v8_value, std::marker::PhantomData))
    }
}

/// A Deserializable javascript value, that can be stored and used later
/// Can only be used on the same runtime it was created on
///
/// This mimics the auto-decoding that happens when providing a type parameter to Runtime functions
#[derive(Eq, Hash, PartialEq, Debug, Clone)]
pub struct Value(V8Value<DefaultTypeChecker>);
impl_v8!(Value, DefaultTypeChecker);
impl Value {
    /// Converts the value to an arbitrary rust type
    /// Mimics the auto-decoding using `from_v8` that normally happens
    /// Note: This will not await the event loop, or resolve promises
    /// Use [`crate::js_value::Promise`] as the generic T for that
    ///
    /// # Errors
    /// Will return an error if the value cannot be deserialized into the given type
    pub fn try_into<T>(self, runtime: &mut crate::Runtime) -> Result<T, crate::Error>
    where
        T: serde::de::DeserializeOwned,
    {
        let context = runtime.deno_runtime().main_context();
        let isolate = runtime.deno_runtime().v8_isolate();
        let scope = std::pin::pin!(v8::HandleScope::new(isolate));
        let mut scope = scope.init();
        let context_local = v8::Local::new(&scope, context);
        let mut context_scope = v8::ContextScope::new(&mut scope, context_local);
        let local = self.0.as_local(&context_scope);
        Ok(deno_core::serde_v8::from_v8(&mut context_scope, local)?)
    }

    /// Contructs a new Value from a `v8::Value` global
    #[must_use]
    pub fn from_v8(value: v8::Global<v8::Value>) -> Self {
        Self(V8Value(value, std::marker::PhantomData))
    }
}

mod function;
pub use function::*;

mod promise;
pub use promise::*;

mod string;
pub use string::*;

mod map;
pub use map::*;

#[cfg(test)]
mod test {
    use super::*;
    use crate::{Module, Runtime, RuntimeOptions};

    #[test]
    fn test_value() {
        let module = Module::new(
            "test.js",
            "
            export const f = 42;
            export const g = () => 42;
        ",
        );

        let mut runtime = Runtime::new(RuntimeOptions::default()).unwrap();
        let handle = runtime.load_module(&module).unwrap();

        // Test basic value extraction
        let f: Value = runtime.get_value(Some(&handle), "f").unwrap();
        let value: usize = f.try_into(&mut runtime).unwrap();
        assert_eq!(value, 42);

        // Test function extraction and conversion
        let g: Value = runtime.get_value(Some(&handle), "g").unwrap();
        let global = g.into_v8();

        // Test try_from_v8 with proper context scope
        let context = runtime.deno_runtime().main_context();
        let isolate = runtime.deno_runtime().v8_isolate();
        let pinned = std::pin::pin!(v8::HandleScope::new(isolate));
        let mut scope = pinned.init();
        let context_local = v8::Local::new(&scope, context);
        let mut context_scope = v8::ContextScope::new(&mut scope, context_local);

        // Test that we can create a Function from the global
        let _f = Function::try_from_v8(&mut context_scope, global.clone()).unwrap();

        // Test from_v8_unchecked
        let f = unsafe { Function::from_v8_unchecked(global.clone()) };

        // Test as_local with context scope
        let _local = f.into_inner().as_local(&context_scope);
    }
}
