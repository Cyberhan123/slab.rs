import { spawn } from "node:child_process";
import type { CommandResult } from "./types.ts";
import { summarizeCommand } from "./utils.ts";

interface RunCommandOptions {
  cwd: string;
  env?: Record<string, string | undefined>;
  timeoutMs: number;
}

export async function runCommand(
  command: string[],
  options: RunCommandOptions,
): Promise<CommandResult> {
  return new Promise<CommandResult>((resolve) => {
    const stdoutChunks: Buffer[] = [];
    const stderrChunks: Buffer[] = [];
    let resolved = false;
    let timedOut = false;

    const child = spawn(command[0] ?? "", command.slice(1), {
      cwd: options.cwd,
      env: {
        ...process.env,
        NO_COLOR: "1",
        CARGO_TERM_COLOR: "never",
        ...options.env,
      },
      stdio: ["ignore", "pipe", "pipe"],
      windowsHide: true,
      shell: false,
    });

    const timeout = setTimeout(() => {
      timedOut = true;
      child.kill();
    }, options.timeoutMs);

    child.stdout.on("data", (chunk: Buffer) => {
      stdoutChunks.push(chunk);
    });

    child.stderr.on("data", (chunk: Buffer) => {
      stderrChunks.push(chunk);
    });

    child.on("error", (error) => {
      if (resolved) {
        return;
      }

      resolved = true;
      clearTimeout(timeout);
      resolve({
        command,
        cwd: options.cwd,
        stdout: Buffer.concat(stdoutChunks).toString("utf8"),
        stderr: Buffer.concat(stderrChunks).toString("utf8"),
        exitCode: null,
        ok: false,
        timedOut,
        errorMessage: error.message,
      });
    });

    child.on("close", (exitCode) => {
      if (resolved) {
        return;
      }

      resolved = true;
      clearTimeout(timeout);
      const stdout = Buffer.concat(stdoutChunks).toString("utf8");
      const stderr = Buffer.concat(stderrChunks).toString("utf8");

      resolve({
        command,
        cwd: options.cwd,
        stdout,
        stderr,
        exitCode,
        ok: exitCode === 0,
        timedOut,
        errorMessage: timedOut
          ? `Command timed out after ${options.timeoutMs}ms: ${summarizeCommand(command)}`
          : undefined,
      });
    });
  });
}
