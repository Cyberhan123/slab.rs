#!/usr/bin/env bun

import { readFile, readdir, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

type BundleBudgetConfig = {
  distDir: string;
  budgets: {
    main: {
      baselineBytes: number;
      maxIncreasePercent: number;
    };
    workspaceLspClient: {
      baselineBytes: number;
    };
    workspaceRoute: {
      baselineBytes: number;
    };
  };
};

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const configPath = path.join(repoRoot, "packages", "slab-desktop", "bundle-budget.json");

async function main() {
  const config = JSON.parse(await readFile(configPath, "utf8")) as BundleBudgetConfig;
  const distDir = path.resolve(repoRoot, config.distDir);
  const assetsDir = path.join(distDir, "assets");
  const indexHtmlPath = path.join(distDir, "index.html");
  const files = await readdir(assetsDir);
  const indexHtml = await readFile(indexHtmlPath, "utf8");

  const mainChunkName = indexHtml.match(/\/assets\/(index-[^"]+\.js)"/)?.[1];
  if (!mainChunkName) {
    throw new Error(`Could not find desktop main JS chunk in ${path.relative(repoRoot, indexHtmlPath)}.`);
  }

  const mainChunk = await chunkSize(assetsDir, mainChunkName);
  const workspaceLsp = await findChunkSize(assetsDir, files, "workspace-lsp-client-");
  const workspaceRoute = await findChunkSize(assetsDir, files, "workspace-", (file) =>
    !file.startsWith("workspace-lsp"),
  );
  const maxMainBytes = Math.ceil(
    config.budgets.main.baselineBytes * (1 + config.budgets.main.maxIncreasePercent / 100),
  );

  const rows = [
    ["main", mainChunk.name, mainChunk.size, config.budgets.main.baselineBytes, maxMainBytes],
    [
      "workspace-lsp-client",
      workspaceLsp.name,
      workspaceLsp.size,
      config.budgets.workspaceLspClient.baselineBytes,
      null,
    ],
    [
      "workspace-route",
      workspaceRoute.name,
      workspaceRoute.size,
      config.budgets.workspaceRoute.baselineBytes,
      null,
    ],
  ] as const;

  for (const [label, file, size, baseline, max] of rows) {
    const delta = size - baseline;
    const pct = baseline > 0 ? (delta / baseline) * 100 : 0;
    const limit = max === null ? "tracked" : `${formatBytes(max)} max`;
    console.log(
      `${label}: ${file} ${formatBytes(size)} (${formatSignedPercent(pct)} vs baseline, ${limit})`,
    );
  }

  if (mainChunk.size > maxMainBytes) {
    throw new Error(
      `Desktop main chunk ${mainChunk.name} is ${formatBytes(mainChunk.size)}, above the Plan F budget ${formatBytes(maxMainBytes)}.`,
    );
  }
}

async function findChunkSize(
  assetsDir: string,
  files: string[],
  prefix: string,
  predicate: (file: string) => boolean = () => true,
) {
  const candidates = files.filter((file) => file.startsWith(prefix) && file.endsWith(".js") && predicate(file));
  if (candidates.length === 0) {
    throw new Error(`Could not find required desktop chunk with prefix ${prefix}.`);
  }

  const entries = await Promise.all(candidates.map((file) => chunkSize(assetsDir, file)));
  return entries.toSorted((a, b) => b.size - a.size)[0];
}

async function chunkSize(assetsDir: string, name: string) {
  const size = (await stat(path.join(assetsDir, name))).size;
  return { name, size };
}

function formatBytes(value: number) {
  return `${(value / 1024).toFixed(1)} KiB`;
}

function formatSignedPercent(value: number) {
  const sign = value >= 0 ? "+" : "";
  return `${sign}${value.toFixed(2)}%`;
}

await main();
