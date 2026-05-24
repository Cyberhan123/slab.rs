use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("missing OUT_DIR"));
    let output = out_dir.join("embedded_stdlib.rs");
    let stdlib_dir = resolve_stdlib_dir();

    match stdlib_dir {
        Some(stdlib_dir) => {
            println!("cargo:rerun-if-changed={}", stdlib_dir.display());
            let modules = collect_stdlib_modules(&stdlib_dir);
            write_embedded_stdlib(&output, &modules).expect("failed to write embedded stdlib");
        }
        None => {
            println!("cargo:warning=Unable to locate Python stdlib; embedded stdlib is empty");
            write_embedded_stdlib(&output, &[]).expect("failed to write empty embedded stdlib");
        }
    }
}

fn resolve_stdlib_dir() -> Option<PathBuf> {
    if let Ok(path) = env::var("SLAB_PYTHON_STDLIB_DIR") {
        let path = PathBuf::from(path);
        if path.is_dir() {
            return Some(path);
        }
    }

    if let Ok(path) = env::var("PYO3_PYTHON")
        && let Some(stdlib) = query_python_stdlib(CommandSpec::new(path))
    {
        return Some(stdlib);
    }

    for command in [
        CommandSpec::new("python"),
        CommandSpec::new("python3"),
        CommandSpec::new("py").with_arg("-3"),
    ] {
        if let Some(stdlib) = query_python_stdlib(command) {
            return Some(stdlib);
        }
    }

    None
}

fn query_python_stdlib(command: CommandSpec) -> Option<PathBuf> {
    let mut child = Command::new(command.program);
    child.args(command.args);
    child.arg("-c");
    child.arg("import sysconfig; print(sysconfig.get_paths().get('stdlib', ''))");
    let output = child.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8(output.stdout).ok()?;
    let path = PathBuf::from(path.trim());
    path.is_dir().then_some(path)
}

struct CommandSpec {
    program: String,
    args: Vec<String>,
}

impl CommandSpec {
    fn new(program: impl Into<String>) -> Self {
        Self { program: program.into(), args: Vec::new() }
    }

    fn with_arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }
}

#[derive(Debug)]
struct EmbeddedModule {
    name: String,
    path: PathBuf,
    is_package: bool,
}

fn collect_stdlib_modules(stdlib_dir: &Path) -> Vec<EmbeddedModule> {
    let mut modules = Vec::new();
    collect_stdlib_modules_inner(stdlib_dir, stdlib_dir, &mut modules);
    modules.sort_by(|left, right| left.name.cmp(&right.name));
    modules
}

fn collect_stdlib_modules_inner(root: &Path, current: &Path, modules: &mut Vec<EmbeddedModule>) {
    let Ok(entries) = fs::read_dir(current) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = entry.file_name();
        if path.is_dir() {
            if should_skip_dir(&file_name) {
                continue;
            }
            collect_stdlib_modules_inner(root, &path, modules);
            continue;
        }
        if !path.is_file() || path.extension().and_then(OsStr::to_str) != Some("py") {
            continue;
        }
        let Some(module) = module_name_for_path(root, &path) else {
            continue;
        };
        modules.push(module);
    }
}

fn should_skip_dir(name: &OsStr) -> bool {
    matches!(
        name.to_str().unwrap_or_default(),
        "__pycache__"
            | "site-packages"
            | "dist-packages"
            | "test"
            | "tests"
            | "idlelib"
            | "tkinter"
            | "turtledemo"
            | "ensurepip"
            | "venv"
    )
}

fn module_name_for_path(root: &Path, path: &Path) -> Option<EmbeddedModule> {
    let relative = path.strip_prefix(root).ok()?;
    let mut parts = relative
        .components()
        .map(|component| component.as_os_str().to_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>()?;
    let file_name = parts.pop()?;
    let stem = file_name.strip_suffix(".py")?;
    let is_package = stem == "__init__";
    if !is_package {
        parts.push(stem.to_owned());
    }
    if parts.is_empty() || !parts.iter().all(|part| is_valid_module_segment(part)) {
        return None;
    }
    Some(EmbeddedModule { name: parts.join("."), path: path.to_path_buf(), is_package })
}

fn is_valid_module_segment(segment: &str) -> bool {
    let mut chars = segment.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn write_embedded_stdlib(path: &Path, modules: &[EmbeddedModule]) -> std::io::Result<()> {
    let mut body = String::from(
        "pub fn default_embedded_stdlib() -> crate::vfs::EmbeddedStdlib {\n    let mut stdlib = crate::vfs::EmbeddedStdlib::default();\n",
    );
    for module in modules {
        let path = module.path.display().to_string();
        let escaped_path = path.replace('\\', "\\\\").replace('"', "\\\"");
        let escaped_name = module.name.replace('"', "\\\"");
        let method = if module.is_package { "add_package" } else { "add_module" };
        body.push_str(&format!(
            "    stdlib.{method}(\"{escaped_name}\", include_bytes!(\"{escaped_path}\"));\n"
        ));
    }
    body.push_str("    stdlib\n}\n");
    fs::write(path, body)
}
