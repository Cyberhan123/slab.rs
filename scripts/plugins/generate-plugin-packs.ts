import { mkdir, readFile, readdir, rm, stat, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { deflateRawSync } from "node:zlib";

import { updatePluginManifestIntegrity } from "../../packages/slab-plugin-sdk/src/integrity.ts";

type CliOptions = {
  outDir: string;
  pluginIds: Set<string>;
  pluginsDir: string;
};

type PluginManifest = {
  id: string;
  integrity?: {
    filesSha256?: Record<string, string>;
  };
  name: string;
  runtime?: {
    ui?: {
      entry?: unknown;
    };
    wasm?: {
      entry?: unknown;
    };
  };
  version: string;
};

type ZipEntry = {
  bytes: Uint8Array;
  path: string;
};

const DEFAULT_OUT_DIR = "plugins/dist";
const PACKAGE_EXTENSION = ".plugin.slab";
const CRC32_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let index = 0; index < 256; index += 1) {
    let value = index;
    for (let bit = 0; bit < 8; bit += 1) {
      value = (value & 1) === 1 ? 0xedb88320 ^ (value >>> 1) : value >>> 1;
    }
    table[index] = value >>> 0;
  }
  return table;
})();

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");

const options = parseArgs(process.argv.slice(2));
const pluginDirs = await discoverPluginDirs(options.pluginsDir, options.pluginIds);

if (pluginDirs.length === 0) {
  throw new Error(`No plugin manifests were found under ${options.pluginsDir}.`);
}

await rm(options.outDir, { force: true, recursive: true });
await mkdir(options.outDir, { recursive: true });

const archivePaths = await Promise.all(
  pluginDirs.map(async (pluginDir) => packagePlugin(pluginDir, options.outDir)),
);

console.log(`Generated ${archivePaths.length} plugin pack(s).`);
for (const archivePath of archivePaths) {
  console.log(`- ${path.relative(repoRoot, archivePath)}`);
}

function parseArgs(argv: string[]): CliOptions {
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

async function packagePlugin(pluginDir: string, outDir: string): Promise<string> {
  await updatePluginManifestIntegrity(pluginDir);

  const manifestPath = path.join(pluginDir, "plugin.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8")) as PluginManifest;

  validateManifest(manifest, pluginDir);

  const runtimeEntries = [manifest.runtime?.ui?.entry, manifest.runtime?.wasm?.entry].filter(
    (entry): entry is string => typeof entry === "string",
  );
  await ensureRuntimeEntriesExist(pluginDir, runtimeEntries);

  const fileMap = manifest.integrity?.filesSha256 ?? {};
  const packageFiles = ["plugin.json", ...Object.keys(fileMap)].toSorted((left, right) =>
    left.localeCompare(right),
  );

  const archiveEntries = await Promise.all(
    packageFiles.map(async (relativePath) => {
      const absolutePath = path.join(pluginDir, fromPosix(relativePath));
      if (!(await isFile(absolutePath))) {
        throw new Error(
          `Plugin '${manifest.id}' references a missing packaged file: ${relativePath}`,
        );
      }

      return {
        bytes: new Uint8Array(await readFile(absolutePath)),
        path: toPosix(path.join(manifest.id, relativePath)),
      };
    }),
  );

  const archiveName = `${manifest.id}-${manifest.version}${PACKAGE_EXTENSION}`;
  const archivePath = path.join(outDir, archiveName);
  await writeFile(archivePath, createZipArchive(archiveEntries));
  return archivePath;
}

async function ensureRuntimeEntriesExist(pluginDir: string, runtimeEntries: string[]) {
  await Promise.all(
    runtimeEntries.map(async (relativePath) => {
      const absolutePath = path.join(pluginDir, fromPosix(relativePath));
      if (!(await isFile(absolutePath))) {
        throw new Error(
          `Plugin '${path.basename(pluginDir)}' is missing runtime asset '${relativePath}'.`,
        );
      }
    }),
  );
}

function validateManifest(manifest: PluginManifest, pluginDir: string): void {
  if (typeof manifest.id !== "string" || manifest.id.trim().length === 0) {
    throw new Error(`Plugin manifest in ${pluginDir} is missing 'id'.`);
  }
  if (typeof manifest.name !== "string" || manifest.name.trim().length === 0) {
    throw new Error(`Plugin '${manifest.id}' is missing 'name'.`);
  }
  if (typeof manifest.version !== "string" || manifest.version.trim().length === 0) {
    throw new Error(`Plugin '${manifest.id}' is missing 'version'.`);
  }
}

function fromPosix(relativePath: string): string {
  return relativePath.split("/").join(path.sep);
}

function toPosix(relativePath: string): string {
  return relativePath.split(path.sep).join("/");
}

async function isFile(filePath: string): Promise<boolean> {
  try {
    return (await stat(filePath)).isFile();
  } catch {
    return false;
  }
}

function createZipArchive(entries: ZipEntry[]): Uint8Array {
  const localChunks: Buffer[] = [];
  const centralChunks: Buffer[] = [];
  let offset = 0;
  let centralDirectorySize = 0;

  for (const entry of entries) {
    const fileName = Buffer.from(entry.path, "utf8");
    const input = Buffer.from(entry.bytes);
    const compressed = deflateRawSync(input);
    const crc = crc32(input);
    const localHeaderOffset = offset;

    const localHeader = Buffer.alloc(30);
    localHeader.writeUInt32LE(0x04034b50, 0);
    localHeader.writeUInt16LE(20, 4);
    localHeader.writeUInt16LE(0x0800, 6);
    localHeader.writeUInt16LE(8, 8);
    localHeader.writeUInt16LE(0, 10);
    localHeader.writeUInt16LE(33, 12);
    localHeader.writeUInt32LE(crc, 14);
    localHeader.writeUInt32LE(compressed.length, 18);
    localHeader.writeUInt32LE(input.length, 22);
    localHeader.writeUInt16LE(fileName.length, 26);
    localHeader.writeUInt16LE(0, 28);
    localChunks.push(localHeader, fileName, compressed);
    offset += localHeader.length + fileName.length + compressed.length;

    const centralHeader = Buffer.alloc(46);
    centralHeader.writeUInt32LE(0x02014b50, 0);
    centralHeader.writeUInt16LE(20, 4);
    centralHeader.writeUInt16LE(20, 6);
    centralHeader.writeUInt16LE(0x0800, 8);
    centralHeader.writeUInt16LE(8, 10);
    centralHeader.writeUInt16LE(0, 12);
    centralHeader.writeUInt16LE(33, 14);
    centralHeader.writeUInt32LE(crc, 16);
    centralHeader.writeUInt32LE(compressed.length, 20);
    centralHeader.writeUInt32LE(input.length, 24);
    centralHeader.writeUInt16LE(fileName.length, 28);
    centralHeader.writeUInt16LE(0, 30);
    centralHeader.writeUInt16LE(0, 32);
    centralHeader.writeUInt16LE(0, 34);
    centralHeader.writeUInt16LE(0, 36);
    centralHeader.writeUInt32LE(0, 38);
    centralHeader.writeUInt32LE(localHeaderOffset, 42);
    centralChunks.push(centralHeader, fileName);
    centralDirectorySize += centralHeader.length + fileName.length;
  }

  const centralDirectoryOffset = offset;
  const endOfCentralDirectory = Buffer.alloc(22);
  endOfCentralDirectory.writeUInt32LE(0x06054b50, 0);
  endOfCentralDirectory.writeUInt16LE(0, 4);
  endOfCentralDirectory.writeUInt16LE(0, 6);
  endOfCentralDirectory.writeUInt16LE(entries.length, 8);
  endOfCentralDirectory.writeUInt16LE(entries.length, 10);
  endOfCentralDirectory.writeUInt32LE(centralDirectorySize, 12);
  endOfCentralDirectory.writeUInt32LE(centralDirectoryOffset, 16);
  endOfCentralDirectory.writeUInt16LE(0, 20);

  return Buffer.concat([...localChunks, ...centralChunks, endOfCentralDirectory]);
}

function crc32(bytes: Uint8Array): number {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc = CRC32_TABLE[(crc ^ byte) & 0xff] ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}
