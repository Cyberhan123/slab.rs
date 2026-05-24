#!/usr/bin/env bun

import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { packPlugin, parsePackArgs } from "./pack";

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  try {
    const archivePath = await packPlugin(parsePackArgs(process.argv.slice(2)));
    console.log(`Generated ${path.relative(process.cwd(), archivePath).replace(/\\/g, "/")}`);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}

export const cliPath = fileURLToPath(import.meta.url);
