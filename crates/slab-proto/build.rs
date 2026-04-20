fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protos = [
        "proto/slab/ipc/v1/common.proto",
        "proto/slab/ipc/v1/ggml/llama.proto",
        "proto/slab/ipc/v1/ggml/whisper.proto",
        "proto/slab/ipc/v1/ggml/diffusion.proto",
        "proto/slab/ipc/v1/candle/transformers.proto",
        "proto/slab/ipc/v1/candle/diffusion.proto",
        "proto/slab/ipc/v1/onnx.proto",
    ];
    for proto in protos {
        println!("cargo:rerun-if-changed={proto}");
    }
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    // SAFETY: this build script is single-threaded and sets PROTOC before invoking
    // downstream build tooling, so no concurrent environment access is introduced here.
    unsafe {
        std::env::set_var("PROTOC", protoc);
    }
    tonic_prost_build::configure()
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .build_server(true)
        .build_client(true)
        .compile_protos(&protos, &["proto"])?;
    Ok(())
}
