use serde_json::json;
use slab_utils::hash::sha256_hex_bytes as hash_bytes_hex;
use std::fs;

use super::SOURCE_KIND_IMPORT_PACK;
use super::package::{ensure_path_within, locate_plugin_root};
use super::plugin_api_base_url_from_bind_address;
use super::scan::{scan_plugin_dir, scan_plugins};
use super::validation::normalize_relative_path;

fn temp_dir(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("slab-plugin-service-{name}-{}", uuid::Uuid::new_v4()))
}

fn write(path: &std::path::Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, content).expect("write file");
}

#[test]
fn normalize_relative_path_rejects_parent_segments() {
    assert!(normalize_relative_path("../plugin.json").is_err());
    assert_eq!(normalize_relative_path("ui/index.html").expect("normalize"), "ui/index.html");
}

#[test]
fn ensure_path_within_accepts_canonical_root_and_absolute_child() {
    let root = temp_dir("ensure-path-within");
    let child = root.join("plugin");
    fs::create_dir_all(&child).expect("create child");

    let canonical_root = root.canonicalize().expect("canonical root");

    ensure_path_within(&child, &canonical_root).expect("child should be within root");
    ensure_path_within(&child.join("missing").join("asset.txt"), &canonical_root)
        .expect("missing descendant should be within root");
    assert!(ensure_path_within(&root.with_file_name("outside-plugin"), &canonical_root).is_err());
}

#[test]
fn plugin_api_base_url_accepts_bare_bind_address() {
    assert_eq!(plugin_api_base_url_from_bind_address("127.0.0.1:51843"), "http://127.0.0.1:51843/");
}

#[test]
fn scan_plugin_dir_validates_integrity() {
    let root = temp_dir("scan");
    let plugin_root = root.join("example-plugin");
    write(&plugin_root.join("ui/index.html"), "<html></html>");
    let html_hash = hash_bytes_hex(b"<html></html>");
    write(
        &plugin_root.join("plugin.json"),
        &serde_json::to_string_pretty(&json!({
            "manifestVersion": 1,
            "id": "example-plugin",
            "name": "Example Plugin",
            "version": "0.1.0",
            "runtime": { "ui": { "entry": "ui/index.html" } },
            "integrity": { "filesSha256": { "ui/index.html": html_hash } },
            "permissions": { "network": { "mode": "blocked", "allowHosts": [] } }
        }))
        .expect("manifest json"),
    );

    let scanned = scan_plugin_dir(&plugin_root, "dev").expect("scan plugin");
    assert!(scanned.valid);
    assert_eq!(scanned.id, "example-plugin");
}

#[test]
fn scan_plugin_dir_rejects_reserved_plugin_id_namespace() {
    let root = temp_dir("scan-reserved-id");
    let plugin_root = root.join("slab-core");
    write(&plugin_root.join("ui/index.html"), "<html></html>");
    let html_hash = hash_bytes_hex(b"<html></html>");
    write(
        &plugin_root.join("plugin.json"),
        &serde_json::to_string_pretty(&json!({
            "manifestVersion": 1,
            "id": "slab-core",
            "name": "Reserved Plugin",
            "version": "0.1.0",
            "runtime": { "ui": { "entry": "ui/index.html" } },
            "integrity": { "filesSha256": { "ui/index.html": html_hash } },
            "permissions": { "network": { "mode": "blocked", "allowHosts": [] } }
        }))
        .expect("manifest json"),
    );

    let scanned = scan_plugin_dir(&plugin_root, "dev").expect("scan plugin");

    assert!(!scanned.valid);
    assert!(scanned.error.as_deref().unwrap().contains("namespace is reserved"));
}

#[test]
fn scan_plugin_dir_accepts_dev_manifest_without_integrity() {
    let root = temp_dir("scan-dev-without-integrity");
    let plugin_root = root.join("example-plugin");
    write(&plugin_root.join("ui/index.html"), "<html></html>");
    write(
        &plugin_root.join("plugin.json"),
        &serde_json::to_string_pretty(&json!({
            "manifestVersion": 1,
            "id": "example-plugin",
            "name": "Example Plugin",
            "version": "0.1.0",
            "runtime": { "ui": { "entry": "ui/index.html" } },
            "permissions": { "network": { "mode": "blocked", "allowHosts": [] } }
        }))
        .expect("manifest json"),
    );

    let scanned = scan_plugin_dir(&plugin_root, "dev").expect("scan plugin");

    assert!(scanned.valid, "{:?}", scanned.error);
}

#[test]
fn scan_plugin_dir_rejects_pack_manifest_without_integrity() {
    let root = temp_dir("scan-pack-without-integrity");
    let plugin_root = root.join("example-plugin");
    write(&plugin_root.join("ui/index.html"), "<html></html>");
    write(
        &plugin_root.join("plugin.json"),
        &serde_json::to_string_pretty(&json!({
            "manifestVersion": 1,
            "id": "example-plugin",
            "name": "Example Plugin",
            "version": "0.1.0",
            "runtime": { "ui": { "entry": "ui/index.html" } },
            "permissions": { "network": { "mode": "blocked", "allowHosts": [] } }
        }))
        .expect("manifest json"),
    );

    let scanned = scan_plugin_dir(&plugin_root, SOURCE_KIND_IMPORT_PACK).expect("scan plugin");

    assert!(!scanned.valid);
    assert!(scanned.error.as_deref().expect("validation error").contains("integrity.filesSha256"));
}

#[test]
fn scan_plugin_dir_accepts_python_backend_entry() {
    let root = temp_dir("scan-python");
    let plugin_root = root.join("python-plugin");
    write(&plugin_root.join("ui/index.html"), "<html></html>");
    write(&plugin_root.join("python/plugin.py"), "def run(params):\n    return params\n");
    let html_hash = hash_bytes_hex(b"<html></html>");
    let python_hash = hash_bytes_hex(b"def run(params):\n    return params\n");
    write(
        &plugin_root.join("plugin.json"),
        &serde_json::to_string_pretty(&json!({
            "manifestVersion": 1,
            "id": "python-plugin",
            "name": "Python Plugin",
            "version": "0.1.0",
            "runtime": {
                "ui": { "entry": "ui/index.html" },
                "python": { "entry": "python/plugin.py" }
            },
            "integrity": {
                "filesSha256": {
                    "ui/index.html": html_hash,
                    "python/plugin.py": python_hash
                }
            },
            "permissions": { "network": { "mode": "blocked", "allowHosts": [] } }
        }))
        .expect("manifest json"),
    );

    let scanned = scan_plugin_dir(&plugin_root, "dev").expect("scan plugin");

    assert!(scanned.valid);
    assert!(scanned.manifest.unwrap().runtime.python.is_some());
}

#[test]
fn scan_plugin_dir_rejects_multiple_callable_runtimes() {
    let root = temp_dir("scan-multiple-callable-runtimes");
    let plugin_root = root.join("multi-runtime");
    write(&plugin_root.join("ui/index.html"), "<html></html>");
    write(&plugin_root.join("dist/plugin.js"), "export function run() {}\n");
    write(&plugin_root.join("python/plugin.py"), "def run(params):\n    return params\n");
    let html_hash = hash_bytes_hex(b"<html></html>");
    let js_hash = hash_bytes_hex(b"export function run() {}\n");
    let python_hash = hash_bytes_hex(b"def run(params):\n    return params\n");
    write(
        &plugin_root.join("plugin.json"),
        &serde_json::to_string_pretty(&json!({
            "manifestVersion": 1,
            "id": "multi-runtime",
            "name": "Multi Runtime",
            "version": "0.1.0",
            "runtime": {
                "ui": { "entry": "ui/index.html" },
                "js": { "entry": "dist/plugin.js" },
                "python": { "entry": "python/plugin.py" }
            },
            "integrity": {
                "filesSha256": {
                    "ui/index.html": html_hash,
                    "dist/plugin.js": js_hash,
                    "python/plugin.py": python_hash
                }
            },
            "permissions": { "network": { "mode": "blocked", "allowHosts": [] } }
        }))
        .expect("manifest json"),
    );

    let scanned = scan_plugin_dir(&plugin_root, "dev").expect("scan plugin");

    assert!(!scanned.valid);
    assert!(scanned.error.as_deref().unwrap().contains("at most one callable runtime"));
}

#[test]
fn scan_plugin_dir_accepts_pack_python_bundle_without_source_entry() {
    let root = temp_dir("scan-python-bundle");
    let plugin_root = root.join("python-plugin");
    write(&plugin_root.join("ui/index.html"), "<html></html>");
    write(&plugin_root.join("python/backend.slabpy"), "{}");
    let html_hash = hash_bytes_hex(b"<html></html>");
    let bundle_hash = hash_bytes_hex(b"{}");
    write(
        &plugin_root.join("plugin.json"),
        &serde_json::to_string_pretty(&json!({
            "manifestVersion": 1,
            "id": "python-plugin",
            "name": "Python Plugin",
            "version": "0.1.0",
            "runtime": {
                "ui": { "entry": "ui/index.html" },
                "python": {
                    "entry": "python/plugin.py",
                    "bundle": "python/backend.slabpy"
                }
            },
            "integrity": {
                "filesSha256": {
                    "ui/index.html": html_hash,
                    "python/backend.slabpy": bundle_hash
                }
            },
            "permissions": { "network": { "mode": "blocked", "allowHosts": [] } }
        }))
        .expect("manifest json"),
    );

    let scanned = scan_plugin_dir(&plugin_root, SOURCE_KIND_IMPORT_PACK).expect("scan plugin");

    assert!(scanned.valid, "{:?}", scanned.error);
}

#[test]
fn scan_plugin_dir_accepts_language_server_contribution() {
    let root = temp_dir("scan-language-server");
    let plugin_root = root.join("lsp-plugin");
    write(&plugin_root.join("ui/index.html"), "<html></html>");
    let html_hash = hash_bytes_hex(b"<html></html>");
    write(
        &plugin_root.join("plugin.json"),
        &serde_json::to_string_pretty(&json!({
            "manifestVersion": 1,
            "id": "lsp-plugin",
            "name": "LSP Plugin",
            "version": "0.1.0",
            "runtime": { "ui": { "entry": "ui/index.html" } },
            "integrity": { "filesSha256": { "ui/index.html": html_hash } },
            "permissions": {
                "network": { "mode": "blocked", "allowHosts": [] },
                "lsp": ["languageServer:declare"]
            },
            "contributes": {
                "languageServers": [{
                    "id": "lsp-plugin.pyright",
                    "languages": ["python"],
                    "transport": { "type": "stdio", "command": "pyright-langserver", "args": ["--stdio"] }
                }]
            }
        }))
        .expect("manifest json"),
    );

    let scanned = scan_plugin_dir(&plugin_root, "dev").expect("scan plugin");

    assert!(scanned.valid);
    let manifest = scanned.manifest.expect("manifest");
    assert_eq!(manifest.contributes.language_servers[0].id, "lsp-plugin.pyright");
}

#[test]
fn scan_plugin_dir_rejects_language_server_without_lsp_permission() {
    let root = temp_dir("scan-language-server-permission");
    let plugin_root = root.join("lsp-plugin");
    write(&plugin_root.join("ui/index.html"), "<html></html>");
    let html_hash = hash_bytes_hex(b"<html></html>");
    write(
        &plugin_root.join("plugin.json"),
        &serde_json::to_string_pretty(&json!({
            "manifestVersion": 1,
            "id": "lsp-plugin",
            "name": "LSP Plugin",
            "version": "0.1.0",
            "runtime": { "ui": { "entry": "ui/index.html" } },
            "integrity": { "filesSha256": { "ui/index.html": html_hash } },
            "permissions": { "network": { "mode": "blocked", "allowHosts": [] } },
            "contributes": {
                "languageServers": [{
                    "id": "lsp-plugin.pyright",
                    "languages": ["python"],
                    "transport": { "type": "stdio", "command": "pyright-langserver", "args": ["--stdio"] }
                }]
            }
        }))
        .expect("manifest json"),
    );

    let scanned = scan_plugin_dir(&plugin_root, "dev").expect("scan plugin");

    assert!(!scanned.valid);
    assert!(scanned.error.as_deref().unwrap().contains("permissions.lsp"));
}

#[test]
fn scan_plugin_dir_accepts_agent_hook_contribution() {
    let root = temp_dir("scan-agent-hook");
    let plugin_root = root.join("hook-plugin");
    write(&plugin_root.join("ui/index.html"), "<html></html>");
    write(&plugin_root.join("dist/plugin.js"), "export function onAgentHook() {}\n");
    let html_hash = hash_bytes_hex(b"<html></html>");
    let js_hash = hash_bytes_hex(b"export function onAgentHook() {}\n");
    write(
        &plugin_root.join("plugin.json"),
        &serde_json::to_string_pretty(&json!({
            "manifestVersion": 1,
            "id": "hook-plugin",
            "name": "Hook Plugin",
            "version": "0.1.0",
            "runtime": {
                "ui": { "entry": "ui/index.html" },
                "js": { "entry": "dist/plugin.js" }
            },
            "integrity": {
                "filesSha256": {
                    "ui/index.html": html_hash,
                    "dist/plugin.js": js_hash
                }
            },
            "permissions": {
                "network": { "mode": "blocked", "allowHosts": [] },
                "agent": ["hook:declare"]
            },
            "contributes": {
                "agentHooks": [{
                    "id": "memory-context",
                    "events": ["on_agent_start", "on_llm_start"],
                    "transport": { "runtime": "javascript", "function": "onAgentHook" }
                }]
            }
        }))
        .expect("manifest json"),
    );

    let scanned = scan_plugin_dir(&plugin_root, "dev").expect("scan plugin");

    assert!(scanned.valid, "{:?}", scanned.error);
    let manifest = scanned.manifest.expect("manifest");
    assert_eq!(manifest.contributes.agent_hooks[0].id, "memory-context");
}

#[test]
fn scan_plugin_dir_rejects_agent_hook_without_agent_permission() {
    let root = temp_dir("scan-agent-hook-permission");
    let plugin_root = root.join("hook-plugin");
    write(&plugin_root.join("ui/index.html"), "<html></html>");
    write(&plugin_root.join("dist/plugin.js"), "export function onAgentHook() {}\n");
    let html_hash = hash_bytes_hex(b"<html></html>");
    let js_hash = hash_bytes_hex(b"export function onAgentHook() {}\n");
    write(
        &plugin_root.join("plugin.json"),
        &serde_json::to_string_pretty(&json!({
            "manifestVersion": 1,
            "id": "hook-plugin",
            "name": "Hook Plugin",
            "version": "0.1.0",
            "runtime": {
                "ui": { "entry": "ui/index.html" },
                "js": { "entry": "dist/plugin.js" }
            },
            "integrity": {
                "filesSha256": {
                    "ui/index.html": html_hash,
                    "dist/plugin.js": js_hash
                }
            },
            "permissions": { "network": { "mode": "blocked", "allowHosts": [] } },
            "contributes": {
                "agentHooks": [{
                    "id": "memory-context",
                    "events": ["on_agent_start"],
                    "transport": { "runtime": "javascript", "function": "onAgentHook" }
                }]
            }
        }))
        .expect("manifest json"),
    );

    let scanned = scan_plugin_dir(&plugin_root, "dev").expect("scan plugin");

    assert!(!scanned.valid);
    assert!(scanned.error.as_deref().unwrap().contains("permissions.agent"));
}

#[test]
fn scan_plugin_dir_rejects_agent_hook_without_declared_runtime() {
    let root = temp_dir("scan-agent-hook-runtime");
    let plugin_root = root.join("hook-plugin");
    write(&plugin_root.join("ui/index.html"), "<html></html>");
    let html_hash = hash_bytes_hex(b"<html></html>");
    write(
        &plugin_root.join("plugin.json"),
        &serde_json::to_string_pretty(&json!({
            "manifestVersion": 1,
            "id": "hook-plugin",
            "name": "Hook Plugin",
            "version": "0.1.0",
            "runtime": { "ui": { "entry": "ui/index.html" } },
            "integrity": { "filesSha256": { "ui/index.html": html_hash } },
            "permissions": {
                "network": { "mode": "blocked", "allowHosts": [] },
                "agent": ["hook:declare"]
            },
            "contributes": {
                "agentHooks": [{
                    "id": "memory-context",
                    "events": ["on_agent_start"],
                    "transport": { "runtime": "javascript", "function": "onAgentHook" }
                }]
            }
        }))
        .expect("manifest json"),
    );

    let scanned = scan_plugin_dir(&plugin_root, "dev").expect("scan plugin");

    assert!(!scanned.valid);
    assert!(scanned.error.as_deref().unwrap().contains("runtime.js"));
}

#[test]
fn locate_plugin_root_accepts_nested_archive_layout() {
    let root = temp_dir("locate");
    let nested = root.join("archive-root").join("example-plugin");
    write(&nested.join("plugin.json"), "{}");
    let located = locate_plugin_root(&root).expect("locate plugin root");
    assert_eq!(located, nested);
}

#[test]
fn scan_plugins_ignores_dist_directory() {
    let root = temp_dir("scan-plugins");
    fs::create_dir_all(root.join("dist")).expect("create dist dir");
    fs::write(root.join("dist").join("example.plugin.slab"), b"pack").expect("write pack");

    let rows = scan_plugins(&root).expect("scan plugins");

    assert!(rows.is_empty());
}

#[test]
fn scan_plugins_ignores_non_plugin_directories() {
    let root = temp_dir("scan-non-plugin-directories");
    fs::create_dir_all(root.join("scripts")).expect("create scripts dir");
    fs::write(root.join("scripts").join("generate-plugin-packs.ts"), b"export {};")
        .expect("write helper");

    let rows = scan_plugins(&root).expect("scan plugins");

    assert!(rows.is_empty());
}
