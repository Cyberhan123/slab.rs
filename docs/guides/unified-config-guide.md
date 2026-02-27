# Unified Configuration Interface Guide

## Design Overview

`slab-core` provides a unified configuration interface (`SlabConfig`) using a builder pattern for type-safe, fluent configuration.

### Core Advantages

1. Unified initialization: all engines are configured through a consistent interface.
2. Lazy loading: dynamic library paths are validated during configuration, while models are loaded only when needed.
3. Type safety: the builder pattern provides compile-time checks.
4. Extensibility: adding a new engine only requires adding a new config structure.

## Basic Usage

### 1. Configure Engines

```rust
use slab_core::config::SlabConfig;
use slab_core::engine::ggml::{LlamaEngineConfig, WhisperEngineConfig};
use slab_llama::{LlamaContextParams, LlamaModelParams};

// Configure Llama engine
let llama_config = LlamaEngineConfig::builder()
    .library_path("path/to/llama.dll") // required dynamic library path
    .num_workers(2) // optional, default is 1
    .model_params(LlamaModelParams::default()) // optional
    .context_params(LlamaContextParams::default()) // optional
    .build();

// Configure Whisper engine
let whisper_config = WhisperEngineConfig::builder()
    .library_path("path/to/whisper.dll")
    .build();

// Build unified config
let config = SlabConfig::builder()
    .llama(llama_config)
    .whisper(whisper_config)
    .build();
```

### 2. Initialize Engines (Dynamic Library Only)

```rust
use slab_core::engine::ggml::llama::LlamaService;
use slab_core::engine::ggml::whisper::WhisperService;

// Initialize from config (loads dynamic libraries, not model files)
let llama_service = LlamaService::from_config(&config.llama.unwrap())?;
let whisper_service = WhisperService::from_config(&config.whisper.unwrap())?;
```

### 3. Load Models On Demand

```rust
use slab_llama::{LlamaContextParams, LlamaModelParams};

// Load a concrete model only when needed
llama_service
    .load_model_with_workers(
        "models/llama-3-8b.gguf",
        LlamaModelParams::default(),
        LlamaContextParams::default(),
        2, // num_workers
    )
    .await?;
```

### 4. Integrate with Orchestrator

```rust
use slab_core::backend::ResourceManager;
use slab_core::runtime::{Orchestrator, PipelineBuilder};

// Create resource manager
let resource_manager = ResourceManager::new();
resource_manager.register_backend("llama", 2); // 2 concurrent slots

// Create orchestrator
let orchestrator = Orchestrator::new(resource_manager);

// Create pipeline
let pipeline = PipelineBuilder::new("chat")
    .gpu_stream("llama-generate", move |input| {
        let service = llama_service.clone();
        async move {
            let session = service.create_session().await?;
            service.append_input(session, input).await?;
            let stream = service.generate_stream(session, 512).await?;
            Ok(BackendReply::Stream(stream))
        }
    })
    .build();

// Submit task
let task_id = orchestrator.submit(pipeline).await?;
```

## Configuration Object Lifecycle

### Phase 1: Build Configuration (Immediate)

```rust
let llama_config = LlamaEngineConfig::builder()
    .library_path("path/to/llama.dll")
    .num_workers(4)
    .build();
// At this stage, only the config object is created. No I/O is performed.
```

### Phase 2: Load Dynamic Library (`from_config`)

```rust
let service = LlamaService::from_config(&llama_config)?;
// Loads dynamic library (dlopen/LoadLibrary)
// Calls backend_init()
// Does not load model file
```

### Phase 3: Load Model (Explicit Call)

```rust
service.load_model_with_workers(
    "models/model.gguf",
    LlamaModelParams::default(),
    LlamaContextParams::default(),
    config.num_workers,
)?;
// Reads model file (can be several GB)
// Starts inference engine and worker threads
```

## Advanced Usage

### Conditional Configuration

```rust
let mut builder = SlabConfig::builder();

// Enable Llama only when needed
if enable_llm {
    builder = builder.llama(
        LlamaEngineConfig::builder()
            .library_path(llama_lib)
            .num_workers(4)
            .build(),
    );
}

// Enable Whisper only when needed
if enable_transcription {
    builder = builder.whisper(
        WhisperEngineConfig::builder()
            .library_path(whisper_lib)
            .build(),
    );
}

let config = builder.build();
```

### Runtime Hot Reload (Future Direction)

```rust
// Current behavior: reload replaces the global singleton
llama_service = LlamaService::reload("new/path/to/llama.dll")?;

// Future direction: manage reload through config objects
let new_config = LlamaEngineConfig::builder()
    .library_path("new/path/to/llama.dll")
    .num_workers(8) // tunable parameter
    .build();

llama_service = LlamaService::reload_from_config(&new_config)?;
```

## Migration Guide

### From Legacy Code

Legacy style:

```rust
let llama_service = LlamaService::init("path/to/llama.dll")?;
llama_service.load_model_with_workers(
    "model.gguf",
    LlamaModelParams::default(),
    LlamaContextParams::default(),
    2,
)?;
```

Recommended style:

```rust
// 1) Define configuration
let config = LlamaEngineConfig::builder()
    .library_path("path/to/llama.dll")
    .num_workers(2)
    .build();

// 2) Initialize service
let llama_service = LlamaService::from_config(&config)?;

// 3) Load model (reusing config values)
llama_service.load_model_with_workers(
    "model.gguf",
    config.model_params.unwrap_or_default(),
    config.context_params.unwrap_or_default(),
    config.num_workers,
)?;
```

### Integrate with `slab-server`

At the `slab-server` layer, configuration can be loaded from files:

```rust
// slab-server/src/config.rs
use serde::{Deserialize, Serialize};
use slab_core::config::SlabConfig;

#[derive(Deserialize)]
struct ServerConfig {
    #[serde(flatten)]
    core: SlabConfig,

    // slab-server specific config
    api_keys: ApiKeyConfig,
    preprocessing: PreprocessConfig,
}

// Load from file
let config: ServerConfig = toml::from_str(&fs::read_to_string("config.toml")?)?;

// Initialize core engines
if let Some(llama_config) = &config.core.llama {
    let service = LlamaService::from_config(llama_config)?;
    // ...
}
```

## Future Extensions

### 1. Config Serialization Support (`serde`)

```rust
// slab-core/src/config.rs
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LlamaEngineConfig {
    pub library_path: PathBuf,
    pub num_workers: usize,
    // ...
}
```

### 2. Config Validation

```rust
impl SlabConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate dynamic library paths
        // Validate parameter ranges
        // ...
    }
}
```

### 3. Config Merge

```rust
let base_config = SlabConfig::builder().llama(llama_config).build();

let override_config = SlabConfig::builder()
    .llama(LlamaEngineConfig::builder().num_workers(8).build())
    .build();

let final_config = base_config.merge(override_config);
```

## Summary

The unified configuration interface follows four principles:

1. Separate configuration from execution: config objects contain metadata only and do not execute loading logic.
2. Use lazy initialization: dynamic libraries are loaded on demand, and models are loaded explicitly.
3. Preserve type safety: builder APIs catch config errors early.
4. Enable composition: config objects can be serialized, merged, and validated.

This design keeps `slab-core` focused on runtime execution while allowing `slab-server` to manage configuration lifecycle and sources (files, environment variables, remote config).
