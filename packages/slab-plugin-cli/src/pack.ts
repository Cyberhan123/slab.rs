import { spawn } from "node:child_process";
import {
  copyFile,
  mkdir,
  mkdtemp,
  readdir,
  readFile,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { deflateRawSync } from "node:zlib";

import { computePluginIntegrity } from "@slab/plugin-sdk/integrity";
import { build as viteBuild } from "vite";

export type PackPluginOptions = {
  outDir: string;
  pluginDir: string;
  pythonRequirements?: string;
};

type PluginManifest = {
  contributes?: {
    agentCapabilities?: Array<{
      exposeAsMcpTool?: boolean;
      kind?: "a2u_surface" | "tool" | "workflow";
    }>;
    agentHooks?: unknown[];
    commands?: unknown[];
    languageServers?: Array<{
      transport?: {
        type?: string;
      };
    }>;
    routes?: unknown[];
    settings?: unknown[];
    sidebar?: unknown[];
  };
  id: string;
  integrity?: {
    filesSha256?: Record<string, string>;
  };
  name: string;
  permissions?: {
    agent?: string[];
    lsp?: string[];
    ui?: string[];
  };
  runtime?: {
    ui?: {
      entry?: unknown;
    };
    wasm?: {
      entry?: unknown;
    };
    js?: {
      entry?: unknown;
    };
    python?: {
      bundle?: unknown;
      entry?: unknown;
    };
  };
  version: string;
};

export type PythonBundle = {
  entryModule: string;
  format: "slab.python.bundle.v1";
  modules: PythonBundleModule[];
  nativeExtensions: string[];
};

type PythonBundleModule = {
  isPackage: boolean;
  name: string;
  sourceBase64: string;
};

type PythonSourceRoot = {
  path: string;
};

type ZipEntry = {
  bytes: Uint8Array;
  path: string;
};

const PACKAGE_EXTENSION = ".plugin.slab";
const PYTHON_NATIVE_EXTENSIONS = new Set([".dll", ".dylib", ".pyd", ".so"]);
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

export async function packPlugin(options: PackPluginOptions): Promise<string> {
  const pluginDir = path.resolve(options.pluginDir);
  const outDir = path.resolve(options.outDir);
  const sourceManifestPath = path.join(pluginDir, "plugin.json");
  const sourceManifest = JSON.parse(await readFile(sourceManifestPath, "utf8")) as PluginManifest;
  validateManifest(sourceManifest, pluginDir);

  await mkdir(outDir, { recursive: true });
  const stagingRoot = await mkdtemp(path.join(os.tmpdir(), "slab-plugin-pack-"));
  try {
    const stagingPluginDir = path.join(stagingRoot, sourceManifest.id);
    await mkdir(stagingPluginDir, { recursive: true });

    const packageManifest = JSON.parse(JSON.stringify(sourceManifest)) as PluginManifest;
    delete packageManifest.integrity;

    await stagePluginFiles(pluginDir, stagingPluginDir, packageManifest, options.pythonRequirements);

    const additionalFiles = runtimePackageFiles(packageManifest);
    if (hasNodePackageLanguageServer(packageManifest)) {
      additionalFiles.push("package.json");
    }
    const filesSha256 = await computePluginIntegrity(stagingPluginDir, additionalFiles);
    packageManifest.integrity = { filesSha256 };
    await writeFile(
      path.join(stagingPluginDir, "plugin.json"),
      `${JSON.stringify(packageManifest, null, 2)}\n`,
      "utf8",
    );

    const packageFiles = ["plugin.json", ...Object.keys(filesSha256)].toSorted((left, right) =>
      left.localeCompare(right),
    );
    const archiveEntries = await Promise.all(
      packageFiles.map(async (relativePath) => ({
        bytes: new Uint8Array(await readFile(path.join(stagingPluginDir, fromPosix(relativePath)))),
        path: toPosix(path.join(sourceManifest.id, relativePath)),
      })),
    );

    const archiveName = `${sourceManifest.id}-${sourceManifest.version}${PACKAGE_EXTENSION}`;
    const archivePath = path.join(outDir, archiveName);
    await writeFile(archivePath, createZipArchive(archiveEntries));
    return archivePath;
  } finally {
    await rm(stagingRoot, { force: true, recursive: true });
  }
}

export function parsePackArgs(argv: string[], cwd = process.cwd()): PackPluginOptions {
  if (argv[0] !== "pack") {
    throw new Error("Usage: slab-plugin-cli pack --plugin-dir <dir> --out-dir <dir>");
  }

  let outDir: string | undefined;
  let pluginDir: string | undefined;
  let pythonRequirements: string | undefined;

  for (let index = 1; index < argv.length; index += 1) {
    const argument = argv[index];
    const value = argv[index + 1];

    switch (argument) {
      case "--out-dir":
        if (!value) {
          throw new Error("--out-dir requires a value.");
        }
        outDir = path.resolve(cwd, value);
        index += 1;
        break;
      case "--plugin-dir":
        if (!value) {
          throw new Error("--plugin-dir requires a value.");
        }
        pluginDir = path.resolve(cwd, value);
        index += 1;
        break;
      case "--python-requirements":
        if (!value) {
          throw new Error("--python-requirements requires a value.");
        }
        pythonRequirements = path.resolve(cwd, value);
        index += 1;
        break;
      default:
        throw new Error(`Unknown argument: ${argument}`);
    }
  }

  if (!pluginDir) {
    throw new Error("--plugin-dir is required.");
  }
  if (!outDir) {
    throw new Error("--out-dir is required.");
  }

  return { outDir, pluginDir, pythonRequirements };
}

async function stagePluginFiles(
  sourcePluginDir: string,
  stagingPluginDir: string,
  manifest: PluginManifest,
  pythonRequirements?: string,
): Promise<void> {
  const uiEntry = asString(manifest.runtime?.ui?.entry);
  if (!uiEntry) {
    throw new Error(`Plugin '${manifest.id}' is missing runtime.ui.entry.`);
  }
  await copyDirectoryIfExists(sourcePluginDir, stagingPluginDir, "ui");
  await ensureStagedFile(stagingPluginDir, uiEntry, `runtime.ui.entry '${uiEntry}'`);

  await copyDirectoryIfExists(sourcePluginDir, stagingPluginDir, "schemas");

  const wasmEntry = asString(manifest.runtime?.wasm?.entry);
  if (wasmEntry) {
    await copyRuntimeFile(sourcePluginDir, stagingPluginDir, wasmEntry);
  }

  const jsEntry = asString(manifest.runtime?.js?.entry);
  if (jsEntry) {
    await stageJsBackend(sourcePluginDir, stagingPluginDir, manifest, jsEntry);
  }

  const pythonEntry = asString(manifest.runtime?.python?.entry);
  if (pythonEntry) {
    await stagePythonBackend(
      sourcePluginDir,
      stagingPluginDir,
      manifest,
      pythonEntry,
      pythonRequirements,
    );
  }

  if (hasNodePackageLanguageServer(manifest)) {
    await copyRuntimeFile(sourcePluginDir, stagingPluginDir, "package.json");
  }
}

async function stageJsBackend(
  sourcePluginDir: string,
  stagingPluginDir: string,
  manifest: PluginManifest,
  jsEntry: string,
): Promise<void> {
  if (isPrebuiltJsEntry(jsEntry)) {
    await copyRuntimeFile(sourcePluginDir, stagingPluginDir, jsEntry);
    return;
  }

  const entryPath = path.join(sourcePluginDir, fromPosix(jsEntry));
  if (!(await isFile(entryPath))) {
    throw new Error(`Plugin '${manifest.id}' is missing JS backend entry '${jsEntry}'.`);
  }

  const outDir = path.join(stagingPluginDir, "dist");
  await viteBuild({
    build: {
      emptyOutDir: true,
      minify: false,
      outDir,
      rollupOptions: {
        output: {
          entryFileNames: "plugin.js",
          format: "es",
          inlineDynamicImports: true,
        },
      },
      ssr: entryPath,
      target: "esnext",
    },
    configFile: false,
    logLevel: "warn",
    root: sourcePluginDir,
    ssr: {
      noExternal: true,
    },
  });

  if (!manifest.runtime?.js) {
    throw new Error(`Plugin '${manifest.id}' has no runtime.js manifest section.`);
  }
  manifest.runtime.js.entry = "dist/plugin.js";
  await ensureStagedFile(stagingPluginDir, "dist/plugin.js", "compiled JS backend");
}

async function stagePythonBackend(
  sourcePluginDir: string,
  stagingPluginDir: string,
  manifest: PluginManifest,
  pythonEntry: string,
  pythonRequirements?: string,
): Promise<void> {
  if (!manifest.runtime?.python) {
    throw new Error(`Plugin '${manifest.id}' has no runtime.python manifest section.`);
  }

  const configuredBundle = asString(manifest.runtime.python.bundle);
  if (configuredBundle) {
    await copyRuntimeFile(sourcePluginDir, stagingPluginDir, configuredBundle);
    return;
  }

  const bundlePath = "python/backend.slabpy";
  const bundle = await buildPythonBundle(sourcePluginDir, pythonEntry, pythonRequirements);
  const absoluteBundlePath = path.join(stagingPluginDir, fromPosix(bundlePath));
  await mkdir(path.dirname(absoluteBundlePath), { recursive: true });
  await writeFile(absoluteBundlePath, `${JSON.stringify(bundle)}\n`, "utf8");
  manifest.runtime.python.bundle = bundlePath;
}

export async function buildPythonBundle(
  sourcePluginDir: string,
  pythonEntry: string,
  pythonRequirements?: string,
): Promise<PythonBundle> {
  const entryPath = path.join(sourcePluginDir, fromPosix(pythonEntry));
  if (!(await isFile(entryPath))) {
    throw new Error(`Python backend entry '${pythonEntry}' does not exist.`);
  }

  const moduleRoot = path.dirname(entryPath);
  const roots: PythonSourceRoot[] = [{ path: moduleRoot }];
  const requirementsPath = await resolvePythonRequirements(sourcePluginDir, pythonRequirements);
  const dependenciesRoot = requirementsPath
    ? await installPythonRequirements(requirementsPath)
    : undefined;
  if (dependenciesRoot) {
    roots.push({ path: dependenciesRoot });
  }

  try {
    const files = (await Promise.all(roots.map((root) => collectFiles(root.path)))).flat();
    const nativeExtensions = files
      .filter((filePath) => PYTHON_NATIVE_EXTENSIONS.has(path.extname(filePath).toLowerCase()))
      .map((filePath) => toPosix(path.relative(sourcePluginDir, filePath)))
      .toSorted((left, right) => left.localeCompare(right));
    if (nativeExtensions.length > 0) {
      throw new Error(
        `Python backend contains native extensions that cannot be bundled yet: ${nativeExtensions.join(
          ", ",
        )}`,
      );
    }

    const modules = (
      await Promise.all(
        roots.map(async (root) => {
          const rootFiles = await collectFiles(root.path);
          return Promise.all(
            rootFiles
              .filter((filePath) => path.extname(filePath).toLowerCase() === ".py")
              .toSorted((left, right) => left.localeCompare(right))
              .map(async (filePath) => ({
                isPackage: path.basename(filePath) === "__init__.py",
                name: pythonModuleName(root.path, filePath),
                sourceBase64: Buffer.from(await readFile(filePath)).toString("base64"),
              })),
          );
        }),
      )
    ).flat();
    const entryModule = pythonModuleName(moduleRoot, entryPath);

    return {
      entryModule,
      format: "slab.python.bundle.v1",
      modules,
      nativeExtensions: [],
    };
  } finally {
    if (dependenciesRoot) {
      await rm(dependenciesRoot, { force: true, recursive: true });
    }
  }
}

async function resolvePythonRequirements(
  sourcePluginDir: string,
  pythonRequirements?: string,
): Promise<string | undefined> {
  if (pythonRequirements) {
    const absolutePath = path.resolve(pythonRequirements);
    if (!(await isFile(absolutePath))) {
      throw new Error(`Python requirements file '${pythonRequirements}' does not exist.`);
    }
    return absolutePath;
  }

  const detected = path.join(sourcePluginDir, "python", "requirements.txt");
  return (await isFile(detected)) ? detected : undefined;
}

async function installPythonRequirements(requirementsPath: string): Promise<string> {
  const targetDir = await mkdtemp(path.join(os.tmpdir(), "slab-plugin-python-deps-"));
  try {
    await runCommand("python", [
      "-m",
      "pip",
      "install",
      "--disable-pip-version-check",
      "--no-build-isolation",
      "--no-compile",
      "--target",
      targetDir,
      "-r",
      requirementsPath,
    ], path.dirname(requirementsPath));
    return targetDir;
  } catch (error) {
    await rm(targetDir, { force: true, recursive: true });
    throw error;
  }
}

async function runCommand(command: string, args: string[], cwd: string): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    const stderr: Buffer[] = [];
    const child = spawn(command, args, {
      cwd,
      env: process.env,
      shell: false,
      stdio: ["ignore", "ignore", "pipe"],
      windowsHide: true,
    });

    child.stderr?.on("data", (chunk) => stderr.push(Buffer.from(chunk)));
    child.once("error", reject);
    child.once("exit", (code) => {
      if (code === 0) {
        resolve();
        return;
      }

      const stderrText = Buffer.concat(stderr).toString("utf8").trim();
      reject(
        new Error(
          `${command} ${args.join(" ")} exited with code ${code ?? "unknown"}${
            stderrText ? `\n${stderrText}` : ""
          }`,
        ),
      );
    });
  });
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
  validateContributionPermissions(manifest);
}

function validateContributionPermissions(manifest: PluginManifest): void {
  const contributes = manifest.contributes ?? {};
  const permissions = manifest.permissions ?? {};
  const uiPermissions = permissions.ui ?? [];
  const agentPermissions = permissions.agent ?? [];
  const lspPermissions = permissions.lsp ?? [];

  const rules: Array<[unknown[] | undefined, string[], string, string]> = [
    [
      contributes.routes,
      uiPermissions,
      "route:create",
      "contributes.routes requires permissions.ui to include route:create",
    ],
    [
      contributes.sidebar,
      uiPermissions,
      "sidebar:item:create",
      "contributes.sidebar requires permissions.ui to include sidebar:item:create",
    ],
    [
      contributes.commands,
      uiPermissions,
      "command:create",
      "contributes.commands requires permissions.ui to include command:create",
    ],
    [
      contributes.settings,
      uiPermissions,
      "settings:section:create",
      "contributes.settings requires permissions.ui to include settings:section:create",
    ],
    [
      contributes.agentCapabilities,
      agentPermissions,
      "capability:declare",
      "contributes.agentCapabilities requires permissions.agent to include capability:declare",
    ],
    [
      contributes.agentHooks,
      agentPermissions,
      "hook:declare",
      "contributes.agentHooks requires permissions.agent to include hook:declare",
    ],
    [
      contributes.languageServers,
      lspPermissions,
      "languageServer:declare",
      "contributes.languageServers requires permissions.lsp to include languageServer:declare",
    ],
  ];

  for (const [items, declaredPermissions, expectedPermission, error] of rules) {
    if ((items?.length ?? 0) > 0 && !declaredPermissions.includes(expectedPermission)) {
      throw new Error(`Plugin '${manifest.id}' ${error}.`);
    }
  }

  if (
    contributes.agentCapabilities?.some((capability) => capability.exposeAsMcpTool === true) &&
    !agentPermissions.includes("mcpTool:expose")
  ) {
    throw new Error(
      `Plugin '${manifest.id}' contributes.agentCapabilities[].exposeAsMcpTool requires permissions.agent to include mcpTool:expose.`,
    );
  }
}

function hasNodePackageLanguageServer(manifest: PluginManifest): boolean {
  return (
    manifest.contributes?.languageServers?.some(
      (provider) => provider.transport?.type === "nodePackage",
    ) ?? false
  );
}

function runtimePackageFiles(manifest: PluginManifest): string[] {
  const entries = [
    asString(manifest.runtime?.ui?.entry),
    asString(manifest.runtime?.wasm?.entry),
    asString(manifest.runtime?.js?.entry),
  ].filter((entry): entry is string => Boolean(entry));
  const python = manifest.runtime?.python;
  const pythonBundle = asString(python?.bundle);
  const pythonEntry = asString(python?.entry);
  if (pythonBundle) {
    entries.push(pythonBundle);
  } else if (pythonEntry) {
    entries.push(pythonEntry);
  }
  return entries.map((entry) => entry.split(/[\\/]+/).join("/"));
}

function asString(value: unknown): string | undefined {
  return typeof value === "string" && value.length > 0 ? value : undefined;
}

function isPrebuiltJsEntry(entry: string): boolean {
  const normalized = entry.split(/[\\/]+/).join("/");
  return normalized.startsWith("dist/") && /\.(mjs|js)$/i.test(normalized);
}

async function copyDirectoryIfExists(
  sourceRoot: string,
  destRoot: string,
  relativeDir: string,
): Promise<void> {
  const sourceDir = path.join(sourceRoot, fromPosix(relativeDir));
  if (!(await isDirectory(sourceDir))) {
    return;
  }

  const files = await collectFiles(sourceDir);
  await Promise.all(
    files.map(async (sourcePath) => {
      const relativePath = path.relative(sourceRoot, sourcePath);
      const destPath = path.join(destRoot, relativePath);
      await mkdir(path.dirname(destPath), { recursive: true });
      await copyFile(sourcePath, destPath);
    }),
  );
}

async function copyRuntimeFile(
  sourceRoot: string,
  destRoot: string,
  relativePath: string,
): Promise<void> {
  const sourcePath = path.join(sourceRoot, fromPosix(relativePath));
  if (!(await isFile(sourcePath))) {
    throw new Error(`Plugin asset '${relativePath}' does not exist.`);
  }

  const destPath = path.join(destRoot, fromPosix(relativePath));
  await mkdir(path.dirname(destPath), { recursive: true });
  await copyFile(sourcePath, destPath);
}

async function ensureStagedFile(root: string, relativePath: string, label: string): Promise<void> {
  if (!(await isFile(path.join(root, fromPosix(relativePath))))) {
    throw new Error(`${label} was not staged at '${relativePath}'.`);
  }
}

async function collectFiles(root: string): Promise<string[]> {
  const rows = await readdir(root, { withFileTypes: true });
  const files = await Promise.all(
    rows.map(async (row) => {
      const absolutePath = path.join(root, row.name);
      if (row.isDirectory()) {
        return collectFiles(absolutePath);
      }
      if (row.isFile()) {
        return [absolutePath];
      }
      return [];
    }),
  );
  return files.flat();
}

function pythonModuleName(moduleRoot: string, filePath: string): string {
  const relativePath = toPosix(path.relative(moduleRoot, filePath));
  const withoutExtension = relativePath.replace(/\.py$/i, "");
  const withoutInit = withoutExtension.endsWith("/__init__")
    ? withoutExtension.slice(0, -"/__init__".length)
    : withoutExtension;
  const moduleName = withoutInit.split("/").filter(Boolean).join(".");
  if (!moduleName) {
    throw new Error(`Python module path '${relativePath}' cannot be bundled as a module.`);
  }
  return moduleName;
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
