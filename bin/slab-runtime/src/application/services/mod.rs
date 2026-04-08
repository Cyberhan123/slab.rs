mod backend_session_service;
mod model_lifecycle_service;
mod runtime_service;

use std::collections::HashMap;
use std::sync::Arc;

use slab_runtime_core::CoreError;
use slab_types::backend::RuntimeBackendId;
use slab_types::{Capability, ModelFamily};
use tokio::sync::RwLock;

use crate::domain::services::{BackendSession, ExecutionHub};
use crate::infra::config::EnabledBackends;

pub(crate) use backend_session_service::BackendSessionService;
pub(crate) use model_lifecycle_service::ModelLifecycleService;
pub use runtime_service::RuntimeApplication;

pub(crate) type SharedRuntimeState = Arc<RwLock<RuntimeState>>;

#[derive(Debug)]
pub(crate) struct RuntimeState {
	pub execution: ExecutionHub,
	pub enabled_backends: EnabledBackends,
	pub sessions: HashMap<BackendKind, BackendSession>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BackendKind {
	Llama,
	Whisper,
	Diffusion,
}

#[derive(Debug)]
pub enum RuntimeApplicationError {
	BackendDisabled(BackendKind),
	Runtime(CoreError),
}

impl RuntimeState {
	pub(crate) fn ensure_enabled(
		&self,
		backend: BackendKind,
	) -> Result<(), RuntimeApplicationError> {
		if backend.is_enabled(&self.enabled_backends) {
			Ok(())
		} else {
			Err(RuntimeApplicationError::BackendDisabled(backend))
		}
	}
}

impl BackendKind {
	pub fn runtime_backend_id(self) -> RuntimeBackendId {
		match self {
			Self::Llama => RuntimeBackendId::GgmlLlama,
			Self::Whisper => RuntimeBackendId::GgmlWhisper,
			Self::Diffusion => RuntimeBackendId::GgmlDiffusion,
		}
	}

	pub fn canonical_id(self) -> &'static str {
		self.runtime_backend_id().canonical_id()
	}

	pub(crate) fn family(self) -> ModelFamily {
		match self {
			Self::Llama => ModelFamily::Llama,
			Self::Whisper => ModelFamily::Whisper,
			Self::Diffusion => ModelFamily::Diffusion,
		}
	}

	pub(crate) fn capability(self) -> Capability {
		match self {
			Self::Llama => Capability::TextGeneration,
			Self::Whisper => Capability::AudioTranscription,
			Self::Diffusion => Capability::ImageGeneration,
		}
	}

	pub(crate) fn is_enabled(self, enabled: &EnabledBackends) -> bool {
		match self {
			Self::Llama => enabled.llama,
			Self::Whisper => enabled.whisper,
			Self::Diffusion => enabled.diffusion,
		}
	}
}

impl std::fmt::Display for RuntimeApplicationError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::BackendDisabled(backend) => {
				write!(f, "{} backend is disabled", backend.canonical_id())
			}
			Self::Runtime(error) => write!(f, "{error}"),
		}
	}
}

impl std::error::Error for RuntimeApplicationError {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		match self {
			Self::BackendDisabled(_) => None,
			Self::Runtime(error) => Some(error),
		}
	}
}

impl From<CoreError> for RuntimeApplicationError {
	fn from(value: CoreError) -> Self {
		Self::Runtime(value)
	}
}
