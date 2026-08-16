// Bind only the whisper.cpp parakeet C API. The header ships inside the
// whisper SDK under vendor/whisper/include; parakeet is a separate shared
// library (parakeet.dll) from whisper.dll, so this -sys crate produces its own
// `ParakeetLib` libloading handle. ggml.h is pulled in transitively by
// parakeet.h. The build.rs allowlists only parakeet_* / PARAKEET_* symbols, so
// the ggml types referenced by the parakeet API surface as opaque types local
// to this crate (no symbol clash with slab-whisper-sys or slab-ggml-sys).
#include "parakeet.h"
