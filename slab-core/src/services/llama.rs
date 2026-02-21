use crate::services;
use slab_llama::{
    LlamaBatch, LlamaContext, LlamaContextParams, LlamaModel, LlamaModelParams, Llama,
    SamplerChainBuilder,
};
use std::env::consts::{DLL_PREFIX, DLL_SUFFIX};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use thiserror::Error;
use tracing::info;

struct LlamaGlobal {
    service: Arc<LlamaService>,
    lib_path: PathBuf,
}

static INSTANCE: OnceLock<RwLock<Option<LlamaGlobal>>> = OnceLock::new();

#[derive(Debug, Error)]
pub enum LlamaServiceError {
    #[error(
        "LlamaService already initialized with different library path: {existing} (requested: {requested})"
    )]
    LibraryPathMismatch { existing: PathBuf, requested: PathBuf },

    #[error("LlamaService global storage not initialized")]
    GlobalStorageNotInitialized,

    #[error("LlamaService instance not initialized")]
    InstanceNotInitialized,

    #[error("Lock poisoned while trying to {operation}")]
    LockPoisoned { operation: &'static str },

    #[error("Model path contains invalid UTF-8")]
    InvalidModelPathUtf8,

    #[error("Llama model not loaded")]
    ModelNotLoaded,

    #[error("Failed to canonicalize llama library path: {path}")]
    CanonicalizeLibraryPath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to initialize llama dynamic library at: {path}")]
    InitializeDynamicLibrary {
        path: PathBuf,
        #[source]
        source: anyhow::Error,
    },

    #[error("Failed to load llama model from: {model_path}")]
    LoadModel {
        model_path: String,
        #[source]
        source: anyhow::Error,
    },

    #[error("Failed to create llama context")]
    CreateContext {
        #[source]
        source: anyhow::Error,
    },

    #[error("Failed to tokenize prompt")]
    TokenizeFailed {
        #[source]
        source: anyhow::Error,
    },

    #[error("Failed to decode batch")]
    DecodeFailed {
        #[source]
        source: anyhow::Error,
    },

    #[error("Failed to convert token to piece")]
    TokenToPieceFailed {
        #[source]
        source: anyhow::Error,
    },
}

/// Holds the loaded model and its inference context.
#[derive(Debug)]
struct LlamaState {
    model: LlamaModel,
    ctx: LlamaContext,
}

#[derive(Debug)]
pub struct LlamaService {
    instance: Arc<Llama>,
    state: Arc<Mutex<Option<LlamaState>>>,
}

// SAFETY: LlamaService is only accessed through Arc<Mutex<...>> for mutable state.
// The `instance: Arc<Llama>` field wraps a dynamically loaded library handle which is
// immutable after creation (models and contexts are created from it, not mutated).
// All mutable inference state is guarded by the `state: Arc<Mutex<...>>` field.
unsafe impl Send for LlamaService {}
unsafe impl Sync for LlamaService {}

impl LlamaService {
    fn resolve_lib_path<P: AsRef<Path>>(path: P) -> Result<PathBuf, services::ServiceError> {
        let llama_lib_name = format!("{}llama{}", DLL_PREFIX, DLL_SUFFIX);

        let mut lib_path = path.as_ref().to_path_buf();
        if lib_path.file_name() != Some(OsStr::new(&llama_lib_name)) {
            lib_path.push(&llama_lib_name);
        }

        std::fs::canonicalize(&lib_path).map_err(|source| {
            LlamaServiceError::CanonicalizeLibraryPath {
                path: lib_path,
                source,
            }
            .into()
        })
    }

    fn build_service(normalized_path: &Path) -> Result<Self, services::ServiceError> {
        info!("current llama path is: {}", normalized_path.display());
        let llama = Llama::new(normalized_path).map_err(|source| {
            LlamaServiceError::InitializeDynamicLibrary {
                path: normalized_path.to_path_buf(),
                source: source.into(),
            }
        })?;

        llama.backend_init();

        Ok(Self {
            instance: Arc::new(llama),
            state: Arc::new(Mutex::new(None)),
        })
    }

    pub fn init<P: AsRef<Path>>(path: P) -> Result<Arc<Self>, services::ServiceError> {
        let normalized_path = Self::resolve_lib_path(path)?;
        let global_lock = INSTANCE.get_or_init(|| RwLock::new(None));

        {
            let read_guard = global_lock
                .read()
                .map_err(|_| LlamaServiceError::LockPoisoned {
                    operation: "read llama global state",
                })?;
            if let Some(global) = read_guard.as_ref() {
                if global.lib_path != normalized_path {
                    return Err(LlamaServiceError::LibraryPathMismatch {
                        existing: global.lib_path.clone(),
                        requested: normalized_path.clone(),
                    }
                    .into());
                }
                return Ok(global.service.clone());
            }
        }

        let service = Arc::new(Self::build_service(&normalized_path)?);
        let mut write_guard = global_lock
            .write()
            .map_err(|_| LlamaServiceError::LockPoisoned {
                operation: "write llama global state",
            })?;

        if let Some(global) = write_guard.as_ref() {
            if global.lib_path != normalized_path {
                return Err(LlamaServiceError::LibraryPathMismatch {
                    existing: global.lib_path.clone(),
                    requested: normalized_path.clone(),
                }
                .into());
            }
            return Ok(global.service.clone());
        }

        *write_guard = Some(LlamaGlobal {
            service: service.clone(),
            lib_path: normalized_path,
        });

        Ok(service)
    }

    pub fn reload<P: AsRef<Path>>(path: P) -> Result<Arc<Self>, services::ServiceError> {
        let normalized_path = Self::resolve_lib_path(path)?;
        let service = Arc::new(Self::build_service(&normalized_path)?);
        let global_lock = INSTANCE.get_or_init(|| RwLock::new(None));
        let mut write_guard = global_lock
            .write()
            .map_err(|_| LlamaServiceError::LockPoisoned {
                operation: "write llama global state",
            })?;

        let previous = write_guard
            .as_ref()
            .map(|g| g.lib_path.display().to_string())
            .unwrap_or_else(|| "<uninitialized>".to_string());

        *write_guard = Some(LlamaGlobal {
            service: service.clone(),
            lib_path: normalized_path.clone(),
        });

        info!(
            "llama service reloaded: {} -> {}",
            previous,
            normalized_path.display()
        );

        Ok(service)
    }

    pub fn current() -> Result<Arc<Self>, services::ServiceError> {
        let global_lock = INSTANCE
            .get()
            .ok_or(LlamaServiceError::GlobalStorageNotInitialized)?;
        let read_guard = global_lock
            .read()
            .map_err(|_| LlamaServiceError::LockPoisoned {
                operation: "read llama global state",
            })?;
        read_guard
            .as_ref()
            .map(|global| global.service.clone())
            .ok_or(LlamaServiceError::InstanceNotInitialized.into())
    }

    /// Load a model from a GGUF file and create an inference context.
    ///
    /// Any previously loaded model and context are replaced.
    pub fn load_model<P: AsRef<Path>>(
        &self,
        path_to_model: P,
        model_params: LlamaModelParams,
        ctx_params: LlamaContextParams,
    ) -> Result<(), services::ServiceError> {
        let mut state_lock =
            self.state
                .lock()
                .map_err(|_| LlamaServiceError::LockPoisoned {
                    operation: "lock llama state",
                })?;
        *state_lock = None;

        let path = path_to_model
            .as_ref()
            .to_str()
            .ok_or(LlamaServiceError::InvalidModelPathUtf8)?;

        let model = self
            .instance
            .load_model_from_file(path, model_params)
            .map_err(|source| LlamaServiceError::LoadModel {
                model_path: path.to_string(),
                source: source.into(),
            })?;

        let ctx = model
            .new_context(ctx_params)
            .map_err(|source| LlamaServiceError::CreateContext {
                source: source.into(),
            })?;

        *state_lock = Some(LlamaState { model, ctx });
        Ok(())
    }

    /// Generate text from a prompt.
    ///
    /// Runs autoregressive decoding until `max_tokens` new tokens are produced
    /// or an end-of-generation token is sampled.  Each call resets the KV cache
    /// so generations are independent.
    pub async fn generate(
        &self,
        prompt: &str,
        max_tokens: usize,
    ) -> Result<String, services::ServiceError> {
        let mut state_lock =
            self.state
                .lock()
                .map_err(|_| LlamaServiceError::LockPoisoned {
                    operation: "lock llama state",
                })?;

        let state = state_lock
            .as_mut()
            .ok_or(LlamaServiceError::ModelNotLoaded)?;

        // Start each generation from a clean KV cache.
        state.ctx.kv_cache_clear();

        // Build a fresh sampler chain for this generation.
        let mut sampler = SamplerChainBuilder::new(self.instance.lib_arc()).build();

        // Tokenize the prompt (add BOS, parse special tokens).
        let tokens = state
            .model
            .tokenize(prompt, true, true)
            .map_err(|source| LlamaServiceError::TokenizeFailed {
                source: source.into(),
            })?;

        if tokens.is_empty() {
            return Ok(String::new());
        }

        // Fill the initial batch with all prompt tokens.
        // Only the last token needs logits (sampling happens there).
        let mut batch = LlamaBatch::new(tokens.len());
        for (i, &token) in tokens.iter().enumerate() {
            batch
                .add(token, i as i32, &[0], i == tokens.len() - 1)
                .map_err(|source| LlamaServiceError::DecodeFailed {
                    source: source.into(),
                })?;
        }

        // Decode the prompt in one shot.
        state
            .ctx
            .decode(&mut batch)
            .map_err(|source| LlamaServiceError::DecodeFailed {
                source: source.into(),
            })?;

        let mut n_cur = tokens.len() as i32;
        let mut output = String::new();

        for _ in 0..max_tokens {
            // Sample from the last logit in the batch (-1 = last position).
            let new_token = sampler.sample(&mut state.ctx, -1);
            sampler.accept(new_token);

            if state.model.token_is_eog(new_token) {
                break;
            }

            let piece = state
                .model
                .token_to_piece(new_token, true)
                .map_err(|source| LlamaServiceError::TokenToPieceFailed {
                    source: source.into(),
                })?;
            output.push_str(&piece);

            // Decode the newly sampled token to advance the context.
            batch.clear();
            batch
                .add(new_token, n_cur, &[0], true)
                .map_err(|source| LlamaServiceError::DecodeFailed {
                    source: source.into(),
                })?;

            state
                .ctx
                .decode(&mut batch)
                .map_err(|source| LlamaServiceError::DecodeFailed {
                    source: source.into(),
                })?;

            n_cur += 1;
        }

        Ok(output)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::services::dylib::DylibService;
    use hf_hub::api::sync::Api;
    use std::path::PathBuf;
    use tokio;

    async fn ensure_llama_dir() -> PathBuf {
        let mut test_data_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        test_data_path.push("../testdata");

        DylibService::new()
            .with_prefix_path(&test_data_path)
            .download_llama()
            .await
            .expect("Failed to download llama")
    }

    #[tokio::test]
    async fn test_llama_current_and_reload() {
        let llama_dir = ensure_llama_dir().await;

        let initial = LlamaService::init(llama_dir.as_path())
            .expect("failed to initialize llama service");
        let current = LlamaService::current().expect("failed to get current llama service");
        assert!(Arc::ptr_eq(&initial, &current));

        let reloaded = LlamaService::reload(llama_dir.as_path())
            .expect("failed to reload llama service");
        let current_after_reload =
            LlamaService::current().expect("failed to get current llama service after reload");

        assert!(Arc::ptr_eq(&reloaded, &current_after_reload));
        assert!(!Arc::ptr_eq(&initial, &reloaded));
    }

    #[tokio::test]
    async fn test_llama_generate() {
        let mut test_data_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        test_data_path.push("../testdata");

        let llama_dir = ensure_llama_dir().await;

        let ls = LlamaService::init(llama_dir.as_path())
            .expect("failed to initialize llama service");

        let api = Api::new().expect("failed to init hf-api");
        let model_path = api
            .model("bartowski/Qwen2.5-0.5B-Instruct-GGUF".into())
            .get("Qwen2.5-0.5B-Instruct-Q4_K_M.gguf")
            .expect("failed to download model");

        ls.load_model(
            model_path.as_path(),
            LlamaModelParams::default(),
            LlamaContextParams::default(),
        )
        .expect("load model failed");

        let result = ls
            .generate("Hello, my name is", 64)
            .await
            .expect("generate failed");

        println!("Generated: {result}");
        assert!(!result.is_empty(), "expected non-empty output");
    }
}
