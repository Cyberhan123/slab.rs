fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto = "proto/slab/ipc/v1/backend.proto";
    println!("cargo:rerun-if-changed={proto}");
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    std::env::set_var("PROTOC", protoc);
    tonic_build::configure()
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .build_server(true)
        .build_client(true)
        .compile_protos(&[proto], &["proto"])?;
    Ok(())
}
