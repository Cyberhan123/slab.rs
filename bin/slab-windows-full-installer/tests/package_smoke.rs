use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, UNIX_EPOCH};

use slab_windows_full_installer::bundle::{
    AssetInput, AssetKind, load_embedded_bundle, read_base_executable_bytes, write_embedded_bundle,
};
use slab_windows_full_installer::installer::{
    NsisInstallerCandidate, full_installer_output_name, is_offline_setup_executable,
    nsis_installer_candidate, select_nsis_installer_candidate,
};
use uuid::Uuid;

#[test]
fn installer_naming_and_candidate_selection_are_stable() {
    let old = UNIX_EPOCH + Duration::from_secs(1);
    let new = UNIX_EPOCH + Duration::from_secs(2);

    assert_eq!(full_installer_output_name("1.2.3"), "Slab_1.2.3_x64-offline-setup.exe");
    assert!(is_offline_setup_executable("Slab_1.2.3_x64-offline-setup.exe"));
    assert!(is_offline_setup_executable("SLAB_1.2.3_X64-OFFLINE-SETUP.EXE"));
    assert!(!is_offline_setup_executable("Slab_1.2.3_x64-setup.exe"));
    assert!(
        nsis_installer_candidate(PathBuf::from("Slab_1.2.3_x64-offline-setup.exe"), old).is_none()
    );
    assert!(nsis_installer_candidate(PathBuf::from("Slab.txt"), old).is_none());
    assert!(nsis_installer_candidate(PathBuf::from("SlabSetup.exe"), old).is_some());

    let selected = select_nsis_installer_candidate([
        NsisInstallerCandidate {
            looks_like_setup: false,
            modified: new,
            path: PathBuf::from("helper.exe"),
        },
        NsisInstallerCandidate {
            looks_like_setup: true,
            modified: old,
            path: PathBuf::from("SlabSetup.exe"),
        },
    ])
    .expect("setup-looking installer should win over newer helper");

    assert_eq!(selected, PathBuf::from("SlabSetup.exe"));
}

#[test]
fn embedded_bundle_round_trips_assets_and_base_executable() {
    let root = env::temp_dir().join(format!("slab-package-bundle-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).expect("create temp root");
    let base_executable = root.join("base.exe");
    let setup_asset = root.join("setup-source.exe");
    let output = root.join("offline.exe");
    let extracted_setup = root.join("extracted").join("setup.exe");
    let extracted_base = root.join("extracted").join("base.exe");

    fs::write(&base_executable, b"base-executable").expect("write base executable");
    fs::write(&setup_asset, b"setup-bytes").expect("write setup asset");

    write_embedded_bundle(
        &base_executable,
        "1.2.3",
        &[AssetInput {
            name: "setup.exe".to_owned(),
            kind: AssetKind::NsisInstaller,
            source_path: setup_asset.clone(),
        }],
        &output,
    )
    .expect("write bundle");

    let bundle =
        load_embedded_bundle(&output).expect("load bundle").expect("bundle should be present");
    assert_eq!(bundle.version(), "1.2.3");
    assert_eq!(bundle.asset_len("setup.exe").expect("asset len"), b"setup-bytes".len() as u64);
    assert_eq!(bundle.base_executable_len(), b"base-executable".len() as u64);
    assert_eq!(bundle.read_asset_bytes("setup.exe").expect("asset bytes"), b"setup-bytes");

    let mut extracted_asset_bytes = 0_u64;
    bundle
        .extract_asset_to_path_with_progress("setup.exe", &extracted_setup, |bytes| {
            extracted_asset_bytes += bytes;
            Ok(())
        })
        .expect("extract setup");
    assert_eq!(extracted_asset_bytes, b"setup-bytes".len() as u64);
    assert_eq!(fs::read(&extracted_setup).expect("read extracted setup"), b"setup-bytes");

    bundle
        .write_base_executable_to_path_with_progress(&extracted_base, |_| Ok(()))
        .expect("write base executable");
    assert_eq!(fs::read(&extracted_base).expect("read extracted base"), b"base-executable");
    assert_eq!(read_base_executable_bytes(&output).expect("read bundled base"), b"base-executable");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn load_embedded_bundle_returns_none_when_footer_is_absent() {
    let root = env::temp_dir().join(format!("slab-package-no-bundle-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).expect("create temp root");
    let executable = root.join("plain.exe");
    fs::write(&executable, b"plain").expect("write plain executable");

    let bundle = load_embedded_bundle(&executable).expect("load plain executable");

    assert!(bundle.is_none());
    let _ = fs::remove_dir_all(&root);
}
