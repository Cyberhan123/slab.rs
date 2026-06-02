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
    output_base = _first_output_line(_bazel_capture(["info", "output_base"], workspace, env))
    if not output_base:
        return None

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

    env = _with_bazel_ffmpeg_env(workspace, os.environ.copy())
    for command in commands:
        cwd = workspace
        if "--cwd" in command:
            index = command.index("--cwd")
            cwd = os.path.join(workspace, command[index + 1])
            del command[index : index + 2]
        code = subprocess.run(command, cwd=cwd, env=env).returncode
        if code != 0:
            return code
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
