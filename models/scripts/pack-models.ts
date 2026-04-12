import { mkdir, readdir, readFile, stat, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { deflateRawSync } from "node:zlib";

type PackResult = {
  manifestId: string | null;
  packDir: string;
  outputPath: string;
  files: number;
};

type ZipEntry = {
  name: string;
  data: Buffer;
};

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const modelsRoot = path.resolve(scriptDir, "..");
const outputDir = path.join(modelsRoot, "dist");
const skippedRoots = new Set(["dist", "node_modules", ".git"]);

async function main() {
  const targets = process.argv.slice(2);
  const packDirs = targets.length > 0 ? await resolveTargetDirs(targets) : await findPackDirs(modelsRoot);

  if (packDirs.length === 0) {
    console.error("No model pack directories with manifest.json were found.");
    process.exitCode = 1;
    return;
  }

  await mkdir(outputDir, { recursive: true });

  const usedNames = new Map<string, number>();
  const results: PackResult[] = [];

  for (const packDir of packDirs) {
    const manifestPath = path.join(packDir, "manifest.json");
    const manifest = JSON.parse(await readFile(manifestPath, "utf8")) as { id?: unknown };
    const files = await collectPackFiles(packDir);
    const zipEntries: ZipEntry[] = await Promise.all(
      files.map(async (filePath) => ({
        name: normalizeArchivePath(path.relative(packDir, filePath)),
        data: Buffer.from(await readFile(filePath)),
      })),
    );
    zipEntries.sort(compareZipEntryNames);

    const outputName = allocatePackName(manifest.id, path.relative(modelsRoot, packDir), usedNames);
    const outputPath = path.join(outputDir, `${outputName}.slab`);

    await writeFile(outputPath, createZipArchive(zipEntries));

    results.push({
      manifestId: typeof manifest.id === "string" ? manifest.id : null,
      packDir,
      outputPath,
      files: zipEntries.length,
    });
  }

  for (const result of results) {
    const relativeSource = path.relative(modelsRoot, result.packDir).split(path.sep).join("/");
    const relativeOutput = path.relative(modelsRoot, result.outputPath).split(path.sep).join("/");
    const label = result.manifestId ?? relativeSource;
    console.log(`Packed ${label} from ${relativeSource} -> ${relativeOutput} (${result.files} files)`);
  }
}

async function resolveTargetDirs(targets: string[]) {
  const resolved: string[] = [];

  for (const target of targets) {
    const candidate = path.resolve(process.cwd(), target);
    const targetStat = await stat(candidate).catch(() => null);
    if (!targetStat) {
      throw new Error(`Target does not exist: ${target}`);
    }

    const packDir = targetStat.isDirectory() ? candidate : path.dirname(candidate);
    const manifestPath = path.join(packDir, "manifest.json");
    const manifestStat = await stat(manifestPath).catch(() => null);
    if (!manifestStat?.isFile()) {
      throw new Error(`Target is not a model pack directory: ${target}`);
    }

    resolved.push(packDir);
  }

  return uniqueSortedPaths(resolved);
}

async function findPackDirs(rootDir: string) {
  const found: string[] = [];
  await walkDirectories(rootDir, async (currentDir) => {
    const entries = await readdir(currentDir, { withFileTypes: true });
    const hasManifest = entries.some((entry) => entry.isFile() && entry.name === "manifest.json");
    if (hasManifest) {
      found.push(currentDir);
      return false;
    }

    return true;
  });

  return uniqueSortedPaths(found);
}

async function walkDirectories(
  currentDir: string,
  visit: (currentDir: string) => Promise<boolean>,
) {
  const shouldContinue = await visit(currentDir);
  if (!shouldContinue) {
    return;
  }

  const entries = await readdir(currentDir, { withFileTypes: true });
  for (const entry of entries) {
    if (!entry.isDirectory()) {
      continue;
    }

    if (currentDir === modelsRoot && skippedRoots.has(entry.name)) {
      continue;
    }

    await walkDirectories(path.join(currentDir, entry.name), visit);
  }
}

async function collectPackFiles(packDir: string) {
  const files: string[] = [];
  await walkPackFiles(packDir, files);
  return files;
}

async function walkPackFiles(currentDir: string, files: string[]) {
  const entries = await readdir(currentDir, { withFileTypes: true });
  for (const entry of entries) {
    const absolutePath = path.join(currentDir, entry.name);
    if (entry.isDirectory()) {
      await walkPackFiles(absolutePath, files);
      continue;
    }

    if (entry.isFile()) {
      files.push(absolutePath);
    }
  }
}

function compareZipEntryNames(left: ZipEntry, right: ZipEntry) {
  if (left.name === "manifest.json" && right.name !== "manifest.json") {
    return -1;
  }
  if (left.name !== "manifest.json" && right.name === "manifest.json") {
    return 1;
  }
  return left.name.localeCompare(right.name);
}

function normalizeArchivePath(relativePath: string) {
  const normalized = relativePath.split(path.sep).join("/").trim();
  const segments = normalized.split("/");

  if (
    normalized.length === 0 ||
    normalized.startsWith("/") ||
    normalized.includes("\\") ||
    segments.some((segment) => segment.length === 0 || segment === "." || segment === "..")
  ) {
    throw new Error(`Invalid archive path: ${relativePath}`);
  }

  return normalized;
}

function allocatePackName(
  manifestId: unknown,
  relativeDir: string,
  usedNames: Map<string, number>,
) {
  const baseName = sanitizePackName(
    typeof manifestId === "string" && manifestId.trim().length > 0
      ? manifestId
      : relativeDir.split(path.sep).join("-"),
  );

  const currentCount = usedNames.get(baseName) ?? 0;
  usedNames.set(baseName, currentCount + 1);

  if (currentCount === 0) {
    return baseName;
  }

  return `${baseName}-${currentCount + 1}`;
}

function sanitizePackName(value: string) {
  return (
    value
      .trim()
      .replace(/[<>:"/\\|?*\u0000-\u001f]+/g, "-")
      .replace(/\s+/g, "-")
      .replace(/-+/g, "-")
      .replace(/^\.+|\.+$/g, "")
      .replace(/^-+|-+$/g, "") || "model-pack"
  );
}

function uniqueSortedPaths(paths: string[]) {
  return [...new Set(paths)].sort((left, right) => left.localeCompare(right));
}

function createZipArchive(entries: ZipEntry[]) {
  const zipParts: Buffer[] = [];
  const centralDirectoryParts: Buffer[] = [];
  let offset = 0;

  for (const entry of entries) {
    const nameBuffer = Buffer.from(entry.name, "utf8");
    const uncompressed = Buffer.isBuffer(entry.data) ? entry.data : Buffer.from(entry.data);
    const compressedCandidate = deflateRawSync(uncompressed);
    const useDeflate = compressedCandidate.length < uncompressed.length;
    const compressed = useDeflate ? compressedCandidate : uncompressed;
    const method = useDeflate ? 8 : 0;
    const crc = crc32(uncompressed);
    const { time, date } = toDosDateTime(new Date());

    const localHeader = Buffer.alloc(30);
    localHeader.writeUInt32LE(0x04034b50, 0);
    localHeader.writeUInt16LE(20, 4);
    localHeader.writeUInt16LE(0, 6);
    localHeader.writeUInt16LE(method, 8);
    localHeader.writeUInt16LE(time, 10);
    localHeader.writeUInt16LE(date, 12);
    localHeader.writeUInt32LE(crc, 14);
    localHeader.writeUInt32LE(compressed.length, 18);
    localHeader.writeUInt32LE(uncompressed.length, 22);
    localHeader.writeUInt16LE(nameBuffer.length, 26);
    localHeader.writeUInt16LE(0, 28);

    zipParts.push(localHeader, nameBuffer, compressed);

    const centralHeader = Buffer.alloc(46);
    centralHeader.writeUInt32LE(0x02014b50, 0);
    centralHeader.writeUInt16LE(20, 4);
    centralHeader.writeUInt16LE(20, 6);
    centralHeader.writeUInt16LE(0, 8);
    centralHeader.writeUInt16LE(method, 10);
    centralHeader.writeUInt16LE(time, 12);
    centralHeader.writeUInt16LE(date, 14);
    centralHeader.writeUInt32LE(crc, 16);
    centralHeader.writeUInt32LE(compressed.length, 20);
    centralHeader.writeUInt32LE(uncompressed.length, 24);
    centralHeader.writeUInt16LE(nameBuffer.length, 28);
    centralHeader.writeUInt16LE(0, 30);
    centralHeader.writeUInt16LE(0, 32);
    centralHeader.writeUInt16LE(0, 34);
    centralHeader.writeUInt16LE(0, 36);
    centralHeader.writeUInt32LE(0, 38);
    centralHeader.writeUInt32LE(offset, 42);

    centralDirectoryParts.push(centralHeader, nameBuffer);
    offset += localHeader.length + nameBuffer.length + compressed.length;
  }

  const centralDirectory = Buffer.concat(centralDirectoryParts);
  const endOfCentralDirectory = Buffer.alloc(22);
  endOfCentralDirectory.writeUInt32LE(0x06054b50, 0);
  endOfCentralDirectory.writeUInt16LE(0, 4);
  endOfCentralDirectory.writeUInt16LE(0, 6);
  endOfCentralDirectory.writeUInt16LE(entries.length, 8);
  endOfCentralDirectory.writeUInt16LE(entries.length, 10);
  endOfCentralDirectory.writeUInt32LE(centralDirectory.length, 12);
  endOfCentralDirectory.writeUInt32LE(offset, 16);
  endOfCentralDirectory.writeUInt16LE(0, 20);

  return Buffer.concat([...zipParts, centralDirectory, endOfCentralDirectory]);
}

function toDosDateTime(date: Date) {
  const year = Math.min(Math.max(date.getFullYear(), 1980), 2107);
  const month = date.getMonth() + 1;
  const day = date.getDate();
  const hours = date.getHours();
  const minutes = date.getMinutes();
  const seconds = Math.floor(date.getSeconds() / 2);

  return {
    time: (hours << 11) | (minutes << 5) | seconds,
    date: ((year - 1980) << 9) | (month << 5) | day,
  };
}

function crc32(buffer: Buffer) {
  let crc = 0xffffffff;
  for (const byte of buffer) {
    crc = (crc >>> 8) ^ CRC32_TABLE[(crc ^ byte) & 0xff];
  }
  return (crc ^ 0xffffffff) >>> 0;
}

const CRC32_TABLE = buildCrc32Table();

function buildCrc32Table() {
  const table = new Uint32Array(256);
  for (let index = 0; index < 256; index += 1) {
    let value = index;
    for (let bit = 0; bit < 8; bit += 1) {
      value = (value & 1) === 1 ? 0xedb88320 ^ (value >>> 1) : value >>> 1;
    }
    table[index] = value >>> 0;
  }
  return table;
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
