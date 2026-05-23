import { createHash } from "node:crypto";
import { readdir, readFile, stat, writeFile } from "node:fs/promises";
import path from "node:path";

export type PluginIntegrityMap = Record<string, string>;

const ALWAYS_INCLUDED_DIRS = ["ui", "schemas"] as const;
const OPTIONAL_INCLUDED_FILES = ["wasm/plugin.wasm", "package.json"] as const;

export async function computePluginIntegrity(
  pluginDir: string,
  additionalFiles: string[] = [],
): Promise<PluginIntegrityMap> {
  const root = path.resolve(pluginDir);
  const directoryEntries = await Promise.all(
    ALWAYS_INCLUDED_DIRS.map(async (relativeDir) => {
      const absoluteDir = path.join(root, relativeDir);
      return (await isDirectory(absoluteDir)) ? collectFiles(root, absoluteDir) : [];
    }),
  );
  const optionalEntries = await Promise.all(
    [...OPTIONAL_INCLUDED_FILES, ...additionalFiles].map(async (relativeFile) => {
      const absoluteFile = path.join(root, relativeFile);
      return (await isFile(absoluteFile)) ? toRelativeKey(root, absoluteFile) : null;
    }),
  );
  const entries = [
    ...directoryEntries.flat(),
    ...optionalEntries.filter((entry): entry is string => entry !== null),
  ];

  const uniqueEntries = [...new Set(entries)].toSorted((a, b) => a.localeCompare(b));
  const hashes = await Promise.all(
    uniqueEntries.map(async (relativePath) => [
      relativePath,
      await sha256File(path.join(root, relativePath)),
    ] as const),
  );
  return Object.fromEntries(hashes);
}

export async function updatePluginManifestIntegrity(pluginDir: string): Promise<PluginIntegrityMap> {
  const root = path.resolve(pluginDir);
  const manifestPath = path.join(root, "plugin.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8")) as Record<string, unknown>;
  const filesSha256 = await computePluginIntegrity(root, manifestRuntimeEntries(manifest));

  manifest.integrity = {
    ...(typeof manifest.integrity === "object" && manifest.integrity
      ? (manifest.integrity as Record<string, unknown>)
      : {}),
    filesSha256,
  };

  await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
  return filesSha256;
}

function manifestRuntimeEntries(manifest: Record<string, unknown>): string[] {
  const runtime = asRecord(manifest.runtime);
  if (!runtime) {
    return [];
  }

  return ["ui", "wasm", "js"]
    .map((runtimeKey) => asRecord(runtime[runtimeKey])?.entry)
    .filter((entry): entry is string => typeof entry === "string" && entry.length > 0)
    .map((entry) => entry.split(/[\\/]+/).join("/"));
}

function asRecord(value: unknown): Record<string, unknown> | undefined {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return undefined;
  }
  return value as Record<string, unknown>;
}

async function collectFiles(root: string, currentDir: string): Promise<string[]> {
  const rows = await readdir(currentDir, { withFileTypes: true });
  const files = await Promise.all(
    rows.map(async (row) => {
      const absolutePath = path.join(currentDir, row.name);
      if (row.isDirectory()) {
        return collectFiles(root, absolutePath);
      }
      if (row.isFile()) {
        return [toRelativeKey(root, absolutePath)];
      }
      return [];
    }),
  );
  return files.flat();
}

async function sha256File(filePath: string): Promise<string> {
  return createHash("sha256").update(await readFile(filePath)).digest("hex");
}

async function isDirectory(filePath: string): Promise<boolean> {
  try {
    return (await stat(filePath)).isDirectory();
  } catch {
    return false;
  }
}

async function isFile(filePath: string): Promise<boolean> {
  try {
    return (await stat(filePath)).isFile();
  } catch {
    return false;
  }
}

function toRelativeKey(root: string, absolutePath: string): string {
  return path.relative(root, absolutePath).split(path.sep).join("/");
}
