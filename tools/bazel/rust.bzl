load("@crates//:defs.bzl", "aliases", "all_crate_deps")
load("@rules_rust//cargo:defs.bzl", "cargo_build_script")
load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_library", "rust_proc_macro")

def _crate_name(name):
    return name.replace("-", "_")

_SLAB_RUST_EDITION = "2024"

def slab_cargo_build_script(
        name = "build_script",
        srcs = ["build.rs"],
        deps = [],
        data = [],
        build_script_env = {}):
    cargo_build_script(
        name = name,
        srcs = srcs,
        aliases = aliases(build = True, build_proc_macro = True),
        build_script_env = build_script_env,
        data = data,
        deps = deps + all_crate_deps(build = True),
        edition = _SLAB_RUST_EDITION,
        proc_macro_deps = all_crate_deps(build_proc_macro = True),
    )

def slab_rust_library(
        name,
        crate_name = None,
        srcs = None,
        deps = [],
        data = [],
        compile_data = [],
        crate_features = [],
        proc_macro = False,
        proc_macro_deps = [],
        build_script = None):
    rule = rust_proc_macro if proc_macro else rust_library
    build_script_deps = [build_script] if build_script else []
    all_deps = deps + build_script_deps + all_crate_deps(normal = True)
    kwargs = {
        "name": name,
        "aliases": aliases(),
        "compile_data": compile_data,
        "edition": _SLAB_RUST_EDITION,
        "crate_features": crate_features,
        "crate_name": crate_name or _crate_name(name),
        "data": data,
        "deps": all_deps,
        "proc_macro_deps": proc_macro_deps + all_crate_deps(proc_macro = True),
        "srcs": srcs or native.glob(["src/**/*.rs"]),
    }
    rule(**kwargs)

def slab_rust_binary(
        name,
        crate_name = None,
        crate_root = "src/main.rs",
        srcs = None,
        deps = [],
        data = [],
        compile_data = [],
        crate_features = [],
        proc_macro_deps = [],
        build_script = None):
    build_script_deps = [build_script] if build_script else []
    all_deps = deps + build_script_deps + all_crate_deps(normal = True)
    kwargs = {
        "name": name,
        "aliases": aliases(),
        "compile_data": compile_data,
        "edition": _SLAB_RUST_EDITION,
        "crate_features": crate_features,
        "crate_name": crate_name or _crate_name(name),
        "crate_root": crate_root,
        "data": data,
        "deps": all_deps,
        "proc_macro_deps": proc_macro_deps + all_crate_deps(proc_macro = True),
        "srcs": srcs or native.glob(["src/**/*.rs"]),
    }
    rust_binary(**kwargs)
