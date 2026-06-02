import os
import platform
import subprocess
import sys


def _split_commands(args: list[str]) -> list[list[str]]:
    commands: list[list[str]] = [[]]
    for arg in args:
        if arg == "--then":
            commands.append([])
            continue
        commands[-1].append(arg)
    return [command for command in commands if command]


def _commands_need_bazel_ffmpeg_env(commands: list[list[str]]) -> bool:
    return any(command[0] in {"bun", "cargo"} for command in commands if command)


def _resolve_command_args(
    command: list[str],
    workspace: str,
    env: dict[str, str],
) -> list[str]:
    resolved: list[str] = []
    for arg in command:
        if not arg.startswith("--bazel-output="):
            resolved.append(arg)
            continue

        label = arg.removeprefix("--bazel-output=")
        resolved.append(
            _bazel_runfile_binary_path(label)
            or _bazel_output_path(label, workspace, env)
            or arg
        )
    return resolved


def _bazel_runfile_binary_path(label: str) -> str | None:
    runfiles_dir = os.environ.get("RUNFILES_DIR")
    if not runfiles_dir or not label.startswith("//") or ":" not in label:
        return None

    package, target = label.removeprefix("//").split(":", 1)
    executable_name = f"{target}.exe" if platform.system() == "Windows" else target
    candidate = os.path.join(runfiles_dir, "_main", package, executable_name)
    return candidate if os.path.isfile(candidate) else None


def _bazel_capture(args: list[str], workspace: str, env: dict[str, str]) -> str | None:
    try:
        result = subprocess.run(
            ["bazelisk", *args],
            cwd=workspace,
            env=env,
            check=True,
            stdout=subprocess.PIPE,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError):
        return None

    return result.stdout


def _first_output_line(output: str | None) -> str | None:
    if not output:
        return None

    for line in output.splitlines():
        candidate = line.strip()
        if candidate:
            return candidate
    return None


def _bazel_output_path(label: str, workspace: str, env: dict[str, str]) -> str | None:
    output = _bazel_capture(["cquery", "--output=files", label], workspace, env)
    candidate = _first_output_line(output)
    if not candidate:
        return None
    return candidate if os.path.isabs(candidate) else os.path.join(workspace, candidate)


def _bazel_external_repo_path(repo_name: str, workspace: str, env: dict[str, str]) -> str | None:
    for output_base in _known_output_base_candidates(env):
        repo_path = _external_repo_path_from_output_base(output_base, repo_name)
        if repo_path:
            return repo_path

    output_base = _first_output_line(_bazel_capture(["info", "output_base"], workspace, env))
    if not output_base:
        return None

    return _external_repo_path_from_output_base(output_base, repo_name)


def _known_output_base_candidates(env: dict[str, str]) -> list[str]:
    candidates = [
        env.get("BAZEL_OUTPUT_BASE"),
        "C:/tmp/b" if platform.system() == "Windows" else None,
    ]
    return [candidate for candidate in candidates if candidate]


def _external_repo_path_from_output_base(output_base: str, repo_name: str) -> str | None:
    candidates = [
        os.path.join(output_base, "external", repo_name),
        os.path.join(output_base, "external", f"+http_archive+{repo_name}"),
    ]
    for candidate in candidates:
        if os.path.isdir(candidate):
            return candidate
    return None


def _ffmpeg_sdk_dir(workspace: str, env: dict[str, str]) -> str | None:
    ffmpeg_dir = _bazel_external_repo_path("ffmpeg_windows_x64", workspace, env)
    if not ffmpeg_dir or not os.path.isdir(ffmpeg_dir):
        ffmpeg_dir = _bazel_output_path("@ffmpeg_windows_x64//:sdk", workspace, env)
    if not ffmpeg_dir:
        return None
    if os.path.isfile(ffmpeg_dir):
        ffmpeg_dir = os.path.dirname(ffmpeg_dir)
    while ffmpeg_dir and not os.path.isdir(os.path.join(ffmpeg_dir, "lib", "pkgconfig")):
        parent = os.path.dirname(ffmpeg_dir)
        if parent == ffmpeg_dir:
            return None
        ffmpeg_dir = parent
    return ffmpeg_dir


def _with_bazel_ffmpeg_env(workspace: str, env: dict[str, str]) -> dict[str, str]:
    if platform.system() != "Windows" or env.get("FFMPEG_DIR"):
        return env

    ffmpeg_dir = _ffmpeg_sdk_dir(workspace, env)
    if not ffmpeg_dir:
        return env

    next_env = env.copy()
    next_env["FFMPEG_DIR"] = ffmpeg_dir

    pkg_config_dir = os.path.join(ffmpeg_dir, "lib", "pkgconfig")
    if os.path.isdir(pkg_config_dir):
        next_env["PKG_CONFIG_PATH"] = os.pathsep.join(
            [pkg_config_dir, next_env.get("PKG_CONFIG_PATH", "")]
        ).rstrip(os.pathsep)

    bin_dir = os.path.join(ffmpeg_dir, "bin")
    if os.path.isdir(bin_dir):
        next_env["PATH"] = os.pathsep.join([bin_dir, next_env.get("PATH", "")])

    return next_env


def main() -> int:
    workspace = os.environ.get("BUILD_WORKSPACE_DIRECTORY")
    if not workspace:
        workspace = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))

    args = sys.argv[1:]

    commands = _split_commands(args)
    if not commands:
        print("workspace_command.py requires a command", file=sys.stderr)
        return 2

    env = os.environ.copy()
    if _commands_need_bazel_ffmpeg_env(commands):
        env = _with_bazel_ffmpeg_env(workspace, env)

    for command in commands:
        cwd = workspace
        if "--cwd" in command:
            index = command.index("--cwd")
            cwd = os.path.join(workspace, command[index + 1])
            del command[index : index + 2]
        command = _resolve_command_args(command, workspace, env)
        code = subprocess.run(command, cwd=cwd, env=env).returncode
        if code != 0:
            return code
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
