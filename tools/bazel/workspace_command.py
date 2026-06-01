import os
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
