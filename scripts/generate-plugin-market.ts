import { createHash } from "node:crypto";
import {
  mkdir,
  readFile,
  readdir,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { deflateRawSync } from "node:zlib";

type CliOptions = {
  outDir: string;
  packageUrlBase?: string;
  pluginIds: Set<string>;
  pluginsDir: string;
  sourceId: string;
};

type PluginIntegrityMap = Record<string, string>;

type MarketManifest = {
  description?: unknown;
  homepage?: unknown;
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
  tags?: unknown;
  version: string;
};

type MarketCatalogItem = {
  description?: string;
  homepage?: string;
  id: string;
  name: string;
  packageSha256: string;
  packageUrl: string;
  tags?: string[];
  version: string;
};

type ZipEntry = {
  bytes: Uint8Array;
  path: string;
};

const ALWAYS_INCLUDED_DIRS = ["ui", "schemas"] as const;
const OPTIONAL_INCLUDED_FILES = ["wasm/plugin.wasm"] as const;
const DEFAULT_OUT_DIR = "plugins/dist";
const DEFAULT_SOURCE_ID = "local-dev";
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
const repoRoot = path.resolve(scriptDir, "..");

const options = parseArgs(process.argv.slice(2));
const pluginDirs = await discoverPluginDirs(options.pluginsDir, options.pluginIds);

if (pluginDirs.length === 0) {
  throw new Error(`No plugin manifests were found under ${options.pluginsDir}.`);
}

await rm(options.outDir, { force: true, recursive: true });
await mkdir(options.outDir, { recursive: true });

const plugins: MarketCatalogItem[] = [];
for (const pluginDir of pluginDirs) {
  plugins.push(await packagePlugin(pluginDir, options.outDir, options.packageUrlBase));
}

plugins.sort((left, right) => left.id.localeCompare(right.id));

const catalogPath = path.join(options.outDir, "plugin-market.json");
await mkdir(options.outDir, { recursive: true });
await writeFile(
  catalogPath,
  `${JSON.stringify({ sourceId: options.sourceId, plugins }, null, 2)}\n`,
  "utf8",
);

console.log(`Generated ${plugins.length} market package(s).`);
console.log(`Catalog: ${catalogPath}`);
console.log(`Archives: ${options.outDir}`);

function parseArgs(argv: string[]): CliOptions {
  const pluginIds = new Set<string>();
  let outDir = path.join(repoRoot, DEFAULT_OUT_DIR);
  let packageUrlBase: string | undefined;
  let pluginsDir = path.join(repoRoot, "plugins");
  let sourceId = DEFAULT_SOURCE_ID;

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
      case "--package-url-base":
        if (!value) {
          throw new Error("--package-url-base requires a value.");
        }
        packageUrlBase = value;
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
      case "--source-id":
        if (!value) {
          throw new Error("--source-id requires a value.");
        }
        sourceId = value;
        index += 1;
        break;
      default:
        throw new Error(`Unknown argument: ${argument}`);
    }
  }

  return {
    outDir,
    packageUrlBase,
    pluginIds,
    pluginsDir,
    sourceId,
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

  const matches: string[] = [];
  for (const pluginDir of directories) {
    const pluginId = path.basename(pluginDir);
    if (pluginIds.size > 0 && !pluginIds.has(pluginId)) {
      continue;
    }

    const manifestPath = path.join(pluginDir, "plugin.json");
    if (await isFile(manifestPath)) {
      matches.push(pluginDir);
    }
  }

  if (pluginIds.size === 0) {
    return matches.sort((left, right) => left.localeCompare(right));
  }

  const discoveredIds = new Set(matches.map((pluginDir) => path.basename(pluginDir)));
  const missingIds = [...pluginIds].filter((pluginId) => !discoveredIds.has(pluginId));
  if (missingIds.length > 0) {
    throw new Error(`Plugin(s) not found: ${missingIds.join(", ")}`);
  }

  return matches.sort((left, right) => left.localeCompare(right));
}

async function packagePlugin(
  pluginDir: string,
  outDir: string,
  packageUrlBase?: string,
): Promise<MarketCatalogItem> {
  await updatePluginManifestIntegrity(pluginDir);

  const manifestPath = path.join(pluginDir, "plugin.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8")) as MarketManifest;

  validateManifest(manifest, pluginDir);

  const runtimeEntries = [
    manifest.runtime?.ui?.entry,
    manifest.runtime?.wasm?.entry,
  ].filter((entry): entry is string => typeof entry === "string");
  await ensureRuntimeEntriesExist(pluginDir, runtimeEntries);

  const fileMap = manifest.integrity?.filesSha256 ?? {};
  const packageFiles = ["plugin.json", ...Object.keys(fileMap)].sort((left, right) =>
    left.localeCompare(right),
  );

  const archiveEntries: Record<string, Uint8Array> = {};
  for (const relativePath of packageFiles) {
    const absolutePath = path.join(pluginDir, fromPosix(relativePath));
    if (!(await isFile(absolutePath))) {
      throw new Error(
        `Plugin '${manifest.id}' references a missing packaged file: ${relativePath}`,
      );
    }
    archiveEntries[toPosix(path.join(manifest.id, relativePath))] = new Uint8Array(
      await readFile(absolutePath),
    );
  }

  const archiveName = `${manifest.id}-${manifest.version}${PACKAGE_EXTENSION}`;
  const archivePath = path.join(outDir, archiveName);
  const archiveBytes = createZipArchive(
    Object.entries(archiveEntries).map(([entryPath, bytes]) => ({
      bytes,
      path: entryPath,
    })),
  );
  await writeFile(archivePath, archiveBytes);

  const item: MarketCatalogItem = {
    id: manifest.id,
    name: manifest.name,
    packageSha256: sha256Hex(archiveBytes),
    packageUrl: resolvePackageUrl(archiveName, archivePath, packageUrlBase),
    version: manifest.version,
  };

  if (typeof manifest.description === "string" && manifest.description.length > 0) {
    item.description = manifest.description;
  }
  if (typeof manifest.homepage === "string" && manifest.homepage.length > 0) {
    item.homepage = manifest.homepage;
  }
  if (Array.isArray(manifest.tags)) {
    const tags = manifest.tags.filter((tag): tag is string => typeof tag === "string");
    if (tags.length > 0) {
      item.tags = tags;
    }
  }

  return item;
}

async function updatePluginManifestIntegrity(
  pluginDir: string,
): Promise<PluginIntegrityMap> {
  const root = path.resolve(pluginDir);
  const manifestPath = path.join(root, "plugin.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8")) as Record<
    string,
    unknown
  >;
  const filesSha256 = await computePluginIntegrity(root);

  manifest.integrity = {
    ...(typeof manifest.integrity === "object" && manifest.integrity
      ? (manifest.integrity as Record<string, unknown>)
      : {}),
    filesSha256,
  };

  await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
  return filesSha256;
}

async function computePluginIntegrity(pluginDir: string): Promise<PluginIntegrityMap> {
  const root = path.resolve(pluginDir);
  const directoryEntries = await Promise.all(
    ALWAYS_INCLUDED_DIRS.map(async (relativeDir) => {
      const absoluteDir = path.join(root, relativeDir);
      return (await isDirectory(absoluteDir)) ? collectFiles(root, absoluteDir) : [];
    }),
  );
  const optionalEntries = await Promise.all(
    OPTIONAL_INCLUDED_FILES.map(async (relativeFile) => {
      const absoluteFile = path.join(root, relativeFile);
      return (await isFile(absoluteFile)) ? toPosix(path.relative(root, absoluteFile)) : null;
    }),
  );

  const entries = [
    ...directoryEntries.flat(),
    ...optionalEntries.filter((entry): entry is string => entry !== null),
  ];
  const uniqueEntries = [...new Set(entries)].sort((left, right) => left.localeCompare(right));
  const hashes = await Promise.all(
    uniqueEntries.map(async (relativePath) => [
      relativePath,
      await sha256File(path.join(root, fromPosix(relativePath))),
    ] as const),
  );
  return Object.fromEntries(hashes);
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
        return [toPosix(path.relative(root, absolutePath))];
      }
      return [];
    }),
  );
  return files.flat();
}

async function ensureRuntimeEntriesExist(
  pluginDir: string,
  runtimeEntries: string[],
): Promise<void> {
  for (const relativePath of runtimeEntries) {
    const absolutePath = path.join(pluginDir, fromPosix(relativePath));
    if (!(await isFile(absolutePath))) {
      throw new Error(
        `Plugin '${path.basename(pluginDir)}' is missing runtime asset '${relativePath}'.`,
      );
    }
  }
}

function resolvePackageUrl(
  archiveName: string,
  archivePath: string,
  packageUrlBase?: string,
): string {
  if (!packageUrlBase) {
    return archiveName;
  }

  const normalizedBase = packageUrlBase.endsWith("/")
    ? packageUrlBase
    : `${packageUrlBase}/`;
  return new URL(archiveName, normalizedBase).href;
}

function validateManifest(manifest: MarketManifest, pluginDir: string): void {
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

async function sha256File(filePath: string): Promise<string> {
  return createHash("sha256").update(await readFile(filePath)).digest("hex");
}

function sha256Hex(bytes: Uint8Array): string {
  return createHash("sha256").update(bytes).digest("hex");
}

function fromPosix(relativePath: string): string {
  return relativePath.split("/").join(path.sep);
}

function toPosix(relativePath: string): string {
  return relativePath.split(path.sep).join("/");
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
