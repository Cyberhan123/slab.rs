fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protos = [
        "proto/slab/ipc/v1/common.proto",
        "proto/slab/ipc/v1/llama.proto",
        "proto/slab/ipc/v1/whisper.proto",
        "proto/slab/ipc/v1/diffusion.proto",
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
        .type_attribute(
            "slab.ipc.v1.ModelLoadRequest.backend_params",
            "#[allow(clippy::large_enum_variant)]",
        )
        .type_attribute(
            "slab.ipc.v1.ModelLoadRequest.BackendParams",
            "#[allow(clippy::large_enum_variant)]",
        )
        .build_server(true)
        .build_client(true)
        .compile_protos(&protos, &["proto"])?;
    Ok(())
}
