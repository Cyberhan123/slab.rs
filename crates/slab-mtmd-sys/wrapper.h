// Bind only the llama.cpp multimodal (mtmd) C API. The headers ship inside the
// llama SDK under vendor/llama/include; mtmd is a separate shared library
// (mtmd.dll) from llama.dll, so this -sys crate produces its own `MtmdLib`
// libloading handle. ggml.h / llama.h are pulled in transitively by mtmd.h —
// they are listed explicitly only for clarity. The build.rs allowlists only
// mtmd_* / MTMD_* symbols, so llama/ggml types referenced by mtmd surface as
// opaque types local to this crate (the safe wrapper casts slab-llama's
// concrete pointers across the boundary).
#include "ggml.h"
#include "llama.h"
#include "mtmd.h"
#include "mtmd-helper.h"
