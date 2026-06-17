import { describe, expect, it } from "vitest";

import { runCommand } from "./command.ts";

describe("runCommand", () => {
  it("captures stdout, stderr, and non-zero exit codes", async () => {
    const result = await runCommand(
      [
        process.execPath,
        "-e",
        "process.stdout.write('out'); process.stderr.write('err'); process.exit(5);",
      ],
      {
        cwd: process.cwd(),
        timeoutMs: 1_000,
      },
    );

    expect(result).toMatchObject({
      exitCode: 5,
      ok: false,
      timedOut: false,
      stdout: "out",
      stderr: "err",
    });
  });

  it("reports missing executables with a null exit code", async () => {
    const result = await runCommand(["definitely-missing-slash-cmd"], {
      cwd: process.cwd(),
      timeoutMs: 1_000,
    });

    expect(result.exitCode).toBeNull();
    expect(result.ok).toBe(false);
    expect(result.timedOut).toBe(false);
    expect(result.errorMessage).toBeTruthy();
  });

  it("marks timed out commands and includes the summarized command", async () => {
    const result = await runCommand(
      [process.execPath, "-e", "setTimeout(() => {}, 1000)"],
      {
        cwd: process.cwd(),
        timeoutMs: 50,
      },
    );

    expect(result.timedOut).toBe(true);
    expect(result.ok).toBe(false);
    expect(result.errorMessage).toContain("Command timed out after 50ms");
    expect(result.errorMessage).toContain(process.execPath);
  });
});
