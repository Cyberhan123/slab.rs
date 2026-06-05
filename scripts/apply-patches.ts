/**
 * apply-patches.ts
 *
 * Downloads original crate sources from crates.io into vendor/ and applies
 * the git-format patch files found in patches/.  This is run automatically
 * during `bun run prepare` so the vendor/ tree never needs to be committed.
 *
 * Usage:  bun run scripts/apply-patches.ts
 *         (called automatically by the "prepare" npm script)
 *
 * Algorithm per patch file (e.g. patches/all-smi-luwen-if-0.7.9.patch):
 *  1. Parse crate name and version from the filename.
 *  2. Skip download if vendor/<name-version>/ already exists.
 *  3. Download <name-version>.crate from static.crates.io.
 *  4. Write the tarball to a temp file and extract it with the system `tar`.
 *  5. Apply the patch with `git apply` inside the extracted directory.
 */

import { existsSync, mkdirSync, readdirSync, rmSync, writeFileSync } from "fs";
import { join, relative, resolve } from "path";
import { spawnSync } from "child_process";
import { tmpdir } from "os";

const ROOT = resolve(import.meta.dir, "..");
const PATCHES_DIR = join(ROOT, "patches");
const VENDOR_DIR = join(ROOT, "vendor");

/**
 * Split a patch basename (without .patch) into crate name and version.
 *
 * The heuristic: find the first `-`-separated segment that begins with a
 * digit – everything before it is the name, everything from that segment
 * onwards is the version.
 *
 * Examples:
 *   all-smi-luwen-if-0.7.9        → name: "all-smi-luwen-if",   version: "0.7.9"
 *   deno_io-0.158.0               → name: "deno_io",             version: "0.158.0"
 *   ed448-0.5.0-rc.3              → name: "ed448",               version: "0.5.0-rc.3"
 *   ed448-goldilocks-0.14.0-pre.10 → name: "ed448-goldilocks",   version: "0.14.0-pre.10"
 */
function parseCrateId(base: string): { name: string; version: string } | null {
  const parts = base.split("-");
  const versionStart = parts.findIndex((p) => /^\d/.test(p));
  if (versionStart <= 0) return null;
  return {
    name: parts.slice(0, versionStart).join("-"),
    version: parts.slice(versionStart).join("-"),
  };
}

function run(
  cmd: string,
  args: string[],
  opts: { cwd?: string } = {}
): boolean {
  const result = spawnSync(cmd, args, {
    cwd: opts.cwd,
    stdio: "inherit",
    shell: false,
  });

  if (result.error) {
    if (result.error.code === "ENOENT") {
      throw new Error(
        `Command '${cmd}' not found. scripts/apply-patches.ts requires tools like 'tar' and 'git' to be installed and available in PATH.`
      );
    }

    throw new Error(`Failed to run ${cmd}: ${result.error.message}`);
  }

  return result.status === 0;
}

function checkGitApply(args: string[]): number {
  const result = spawnSync("git", args, {
    cwd: ROOT,
    stdio: "pipe",
    shell: false,
  });

  if (result.error) {
    if (result.error.code === "ENOENT") {
      throw new Error(
        "Command 'git' not found. scripts/apply-patches.ts requires tools like 'tar' and 'git' to be installed and available in PATH."
      );
    }

    throw new Error(`Failed to run git: ${result.error.message}`);
  }

  return result.status ?? 1;
}

async function downloadCrate(name: string, version: string): Promise<Buffer> {
  const url = `https://static.crates.io/crates/${name}/${name}-${version}.crate`;
  console.log(`  Downloading ${url} …`);
  const res = await fetch(url);
  if (!res.ok) throw new Error(`HTTP ${res.status} fetching ${url}`);
  return Buffer.from(await res.arrayBuffer());
}

function extractCrate(
  crateBuffer: Buffer,
  destParent: string,
  dirName: string
): void {
  const tmp = join(tmpdir(), `${dirName}.crate`);
  writeFileSync(tmp, crateBuffer);

  try {
    mkdirSync(destParent, { recursive: true });
    // .crate files are gzip-compressed tar archives
    if (!run("tar", ["-xzf", tmp, "-C", destParent])) {
      throw new Error("tar extraction failed");
    }
  } finally {
    try {
      rmSync(tmp);
    } catch {
      // ignore cleanup errors
    }
  }
}

function applyPatch(vendorDir: string, patchAbsPath: string): void {
  console.log(`  Applying patch …`);

  const vendorRelPath = relative(ROOT, vendorDir).replace(/\\/g, "/");
  const baseArgs = [
    "apply",
    "--ignore-whitespace",
    "-p1",
    `--directory=${vendorRelPath}`,
  ];

  // crates.io archives use LF; patch files can be CRLF on Windows checkouts.
  if (checkGitApply([...baseArgs, "--check", patchAbsPath]) === 0) {
    if (!run("git", [...baseArgs, patchAbsPath], { cwd: ROOT })) {
      throw new Error(`Failed to apply patch ${patchAbsPath} to ${vendorDir}`);
    }
    return;
  }

  if (checkGitApply([...baseArgs, "--reverse", "--check", patchAbsPath]) === 0) {
    console.log("  Patch already applied – skipping.");
    return;
  }

  throw new Error(`Failed to apply patch ${patchAbsPath} to ${vendorDir}`);
}

async function main(): Promise<void> {
  if (!existsSync(PATCHES_DIR)) {
    console.log("No patches/ directory found – nothing to do.");
    return;
  }

  const patchFiles = readdirSync(PATCHES_DIR)
    .filter((file) => file.endsWith(".patch"))
    .toSorted();

  if (patchFiles.length === 0) {
    console.log("No .patch files found in patches/ – nothing to do.");
    return;
  }

  mkdirSync(VENDOR_DIR, { recursive: true });

  let errors = 0;

  for (const patchFile of patchFiles) {
    const base = patchFile.replace(/\.patch$/, "");
    const parsed = parseCrateId(base);
    if (!parsed) {
      console.warn(`[warn] Cannot parse crate id from filename: ${patchFile}`);
      errors++;
      continue;
    }

    const { name, version } = parsed;
    const dirName = `${name}-${version}`;
    const vendorDir = join(VENDOR_DIR, dirName);
    const patchAbsPath = join(PATCHES_DIR, patchFile);

    console.log(`\n[${dirName}]`);

    if (!existsSync(vendorDir)) {
      console.log(`  Fetching crate ${name} v${version} from crates.io …`);
      let buf: Buffer;
      try {
        // Keep downloads sequential so each crate's extraction and patch errors stay scoped.
        // eslint-disable-next-line no-await-in-loop
        buf = await downloadCrate(name, version);
      } catch (err) {
        console.error(`[error] Download failed: ${err}`);
        errors++;
        continue;
      }

      console.log(`  Extracting into vendor/ …`);
      try {
        extractCrate(buf, VENDOR_DIR, dirName);
      } catch (err) {
        if (existsSync(vendorDir)) {
          rmSync(vendorDir, { recursive: true, force: true });
        }
        console.error(`[error] Extraction failed: ${err}`);
        errors++;
        continue;
      }
    } else {
      console.log(`  vendor/${dirName} already present – skipping download.`);
    }

    try {
      applyPatch(vendorDir, patchAbsPath);
    } catch (err) {
      console.error(`[error] ${err}`);
      errors++;
    }
  }

  console.log("\nDone applying patches.");
  if (errors > 0) process.exit(1);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
