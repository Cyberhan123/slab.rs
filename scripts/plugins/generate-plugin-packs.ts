#!/usr/bin/env bun

import { mkdir, readdir, rm, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { packPlugin } from "../../packages/slab-plugin-cli/src/index.ts";

export type CliOptions = {
  outDir: string;
  pluginIds: Set<string>;
  pluginsDir: string;
};

const DEFAULT_OUT_DIR = "plugins/dist";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");

if (isMainModule()) {
  const archivePaths = await generatePluginPacks(parseArgs(process.argv.slice(2)));
  console.log(`Generated ${archivePaths.length} plugin pack(s).`);
  for (const archivePath of archivePaths) {
    console.log(`- ${path.relative(repoRoot, archivePath).replace(/\\/g, "/")}`);
  }
}

export async function generatePluginPacks(options: CliOptions): Promise<string[]> {
  const pluginDirs = await discoverPluginDirs(options.pluginsDir, options.pluginIds);

  if (pluginDirs.length === 0) {
    throw new Error(`No plugin manifests were found under ${options.pluginsDir}.`);
  }

  await rm(options.outDir, { force: true, recursive: true });
  await mkdir(options.outDir, { recursive: true });

  return Promise.all(
    pluginDirs.map((pluginDir) => packPlugin({ outDir: options.outDir, pluginDir })),
  );
}

export function parseArgs(argv: string[]): CliOptions {
  const pluginIds = new Set<string>();
  let outDir = path.join(repoRoot, DEFAULT_OUT_DIR);
  let pluginsDir = path.join(repoRoot, "plugins");

  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    const value = argv[index + 1];

    switch (argument) {
      case "--out-dir":
        if (!value) {
          throw new Error("--out-dir requires a value.");
        }
        outDir = path.resolve(repoRoot, value);
        index += 1;
        break;
      case "--plugin":
        if (!value) {
          throw new Error("--plugin requires a value.");
        }
        pluginIds.add(value);
        index += 1;
        break;
      case "--plugins-dir":
        if (!value) {
          throw new Error("--plugins-dir requires a value.");
        }
        pluginsDir = path.resolve(repoRoot, value);
        index += 1;
        break;
      default:
        throw new Error(`Unknown argument: ${argument}`);
    }
  }

  return {
    outDir,
    pluginIds,
    pluginsDir,
  };
}

async function discoverPluginDirs(
  pluginsDir: string,
  pluginIds: Set<string>,
): Promise<string[]> {
  const rows = await readdir(pluginsDir, { withFileTypes: true });
  const directories = rows
    .filter((row) => row.isDirectory())
    .map((row) => path.join(pluginsDir, row.name));
  const candidates = await Promise.all(
    directories.map(async (pluginDir) => {
      const pluginId = path.basename(pluginDir);
      if (pluginIds.size > 0 && !pluginIds.has(pluginId)) {
        return null;
      }

      const manifestPath = path.join(pluginDir, "plugin.json");
      return (await isFile(manifestPath)) ? pluginDir : null;
    }),
  );
  const matches = candidates.filter((pluginDir): pluginDir is string => pluginDir !== null);

  if (pluginIds.size === 0) {
    return matches.toSorted((left, right) => left.localeCompare(right));
  }

  const discoveredIds = new Set(matches.map((pluginDir) => path.basename(pluginDir)));
  const missingIds = [...pluginIds].filter((pluginId) => !discoveredIds.has(pluginId));
  if (missingIds.length > 0) {
    throw new Error(`Plugin(s) not found: ${missingIds.join(", ")}`);
  }

  return matches.toSorted((left, right) => left.localeCompare(right));
}

async function isFile(filePath: string): Promise<boolean> {
  try {
    return (await stat(filePath)).isFile();
  } catch {
    return false;
  }
}

function isMainModule(): boolean {
  return Boolean(process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href);
}
