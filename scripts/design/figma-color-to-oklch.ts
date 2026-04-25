#!/usr/bin/env bun

import fs from "node:fs/promises";
import path from "node:path";

import postcss from "postcss";
import { converter } from "culori";

type ColorEntry = {
  name: string;
  source: string;
};

type CliOptions = {
  filePath?: string;
  outPath?: string;
  write: boolean;
  useStdin: boolean;
  json: boolean;
  includeComments: boolean;
  annotate: boolean;
  reason: string;
};

type ConversionRecord = {
  prop: string;
  line?: number;
  figmaRefs: string[];
  originalValue: string;
  convertedValue: string;
};

const DEFAULT_REASON =
  "Figma does not currently provide OKLCH in our workflow, so code stores the token in OKLCH for theme compatibility.";

const toOklch = converter("oklch");
const colorRegex = /#(?:[A-Fa-f0-9]{3,4}|[A-Fa-f0-9]{6}|[A-Fa-f0-9]{8})\b|rgba?\(\s*[^)]+\)/g;

function printHelp() {
  console.log(`figma-color-to-oklch

Usage:
  bun run color:oklch -- background=#f7f9fb primary=#0d9488
  bun run color:oklch -- "--background: #f7f9fb;" "--primary: rgb(13, 148, 136);"
  bun run color:oklch -- --stdin < colors.txt
  bun run color:oklch -- --file packages/slab-components/src/styles/globals.css --out packages/slab-components/src/styles/globals.oklch.css
  bun run color:oklch -- --file packages/slab-components/src/styles/globals.css --write --annotate

Options:
  --file PATH      Convert a CSS file with PostCSS.
  --out PATH       Write converted CSS to a separate file.
  --write          Overwrite the input file. If --out is present, writes there.
  --stdin          Read token entries from stdin, one per line.
  --json           Print structured JSON instead of CSS.
  --annotate       Insert compatibility comments before converted custom properties in CSS mode.
  --no-comments    Omit comments in token mode.
  --reason TEXT    Override the compatibility note text.
  -h, --help       Show this help message.

Accepted token inputs:
  background=#f7f9fb
  --background: #f7f9fb;
  primary=rgb(13, 148, 136)
  overlay=rgba(13, 148, 136, 0.72)
`);
}

function unique(values: string[]) {
  return [...new Set(values)];
}

function formatNumber(value: number, digits: number) {
  return value.toFixed(digits).replace(/\.?0+$/, "");
}

function normalizeColorRef(value: string) {
  const trimmed = value.trim();

  if (trimmed.startsWith("#")) {
    return `#${trimmed.slice(1).toUpperCase()}`;
  }

  return trimmed.replace(/\s+/g, " ");
}

function formatOklchValue(input: string) {
  const color = toOklch(input);
  if (!color) {
    return null;
  }

  const lightness = `${formatNumber(color.l * 100, 2)}%`;
  const chroma = formatNumber(color.c ?? 0, 4);
  const hue = formatNumber(color.h ?? 0, 2);
  const alpha =
    color.alpha !== undefined && color.alpha < 0.9995
      ? ` / ${formatNumber(color.alpha, 3)}`
      : "";

  return `oklch(${lightness} ${chroma} ${hue}${alpha})`;
}

function convertCssValue(value: string) {
  const refs: string[] = [];

  const convertedValue = value.replace(colorRegex, (match) => {
    const figmaRef = normalizeColorRef(match);
    const converted = formatOklchValue(figmaRef);
    if (!converted) {
      return match;
    }

    refs.push(figmaRef);
    return converted;
  });

  return {
    changed: convertedValue !== value,
    convertedValue,
    figmaRefs: unique(refs),
  };
}

function hasGeneratedComment(prev: postcss.ChildNode | undefined) {
  return prev?.type === "comment" && prev.text.includes("Figma ref");
}

function buildComment(figmaRefs: string[], reason: string) {
  const refLabel = figmaRefs.length === 1 ? "Figma ref" : "Figma refs";
  return postcss.comment({
    text: `${refLabel}: ${figmaRefs.join(", ")}\n   Reason: ${reason}\n `,
  });
}

async function processCss(css: string, options: CliOptions, from = "<memory>", to = "<memory>") {
  const records: ConversionRecord[] = [];

  const plugin = {
    postcssPlugin: "figma-color-to-oklch",
    Declaration(decl: postcss.Declaration) {
      const { changed, convertedValue, figmaRefs } = convertCssValue(decl.value);
      if (!changed) {
        return;
      }

      const originalValue = decl.value;
      decl.value = convertedValue;

      records.push({
        prop: decl.prop,
        line: decl.source?.start?.line,
        figmaRefs,
        originalValue,
        convertedValue,
      });

      if (
        options.annotate &&
        options.includeComments &&
        decl.prop.startsWith("--") &&
        figmaRefs.length > 0 &&
        !hasGeneratedComment(decl.prev())
      ) {
        decl.parent?.insertBefore(decl, buildComment(figmaRefs, options.reason));
      }
    },
  };

  const result = await postcss([plugin]).process(css, { from, to });
  return { css: result.css, records, root: result.root };
}

function parseEntry(raw: string, index: number): ColorEntry | null {
  const trimmed = raw.trim();
  if (!trimmed || trimmed.startsWith("//")) {
    return null;
  }

  const assignment = trimmed.match(/^(--)?([A-Za-z0-9_-]+)\s*[:=]\s*(.+?)\s*;?$/);
  if (assignment) {
    return {
      name: assignment[2],
      source: assignment[3],
    };
  }

  return {
    name: `color-${index}`,
    source: trimmed.replace(/;$/, ""),
  };
}

async function readStdin() {
  if (process.stdin.isTTY) {
    return "";
  }

  return await Bun.stdin.text();
}

function parseArgs(argv: string[]) {
  const options: CliOptions = {
    write: false,
    useStdin: false,
    json: false,
    includeComments: true,
    annotate: false,
    reason: DEFAULT_REASON,
  };
  const rawEntries: string[] = [];

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];

    if (arg === "-h" || arg === "--help") {
      printHelp();
      process.exit(0);
    }

    if (arg === "--") {
      continue;
    }

    if (arg === "--stdin") {
      options.useStdin = true;
      continue;
    }

    if (arg === "--json") {
      options.json = true;
      continue;
    }

    if (arg === "--annotate") {
      options.annotate = true;
      continue;
    }

    if (arg === "--no-comments") {
      options.includeComments = false;
      continue;
    }

    if (arg === "--write") {
      options.write = true;
      continue;
    }

    if (arg === "--reason") {
      const next = argv[index + 1];
      if (!next) {
        throw new Error("--reason requires a value");
      }

      options.reason = next;
      index += 1;
      continue;
    }

    if (arg === "--file") {
      const next = argv[index + 1];
      if (!next) {
        throw new Error("--file requires a value");
      }

      options.filePath = next;
      index += 1;
      continue;
    }

    if (arg === "--out") {
      const next = argv[index + 1];
      if (!next) {
        throw new Error("--out requires a value");
      }

      options.outPath = next;
      index += 1;
      continue;
    }

    rawEntries.push(arg);
  }

  return { options, rawEntries };
}

async function handleTokenMode(options: CliOptions, rawEntries: string[]) {
  const stdinText = options.useStdin ? await readStdin() : "";
  const inputLines = [
    ...rawEntries,
    ...stdinText
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean),
  ];

  if (inputLines.length === 0) {
    printHelp();
    process.exit(1);
  }

  const entries = inputLines
    .map((line, index) => parseEntry(line, index + 1))
    .filter((entry): entry is ColorEntry => entry !== null);

  if (entries.length === 0) {
    throw new Error("No color entries found.");
  }

  const syntheticCss = `:root {\n${entries
    .map((entry) => `  --${entry.name}: ${entry.source};`)
    .join("\n")}\n}\n`;

  const syntheticOptions = {
    ...options,
    annotate: options.includeComments,
  };
  const { root, records } = await processCss(syntheticCss, syntheticOptions);
  const rule = root.first;

  if (options.json) {
    console.log(JSON.stringify(records, null, 2));
    return;
  }

  if (!rule || rule.type !== "rule" || !rule.nodes) {
    throw new Error("Failed to build token output.");
  }

  const output = root
    .toString()
    .replace(/^:root\s*\{/, "")
    .replace(/\}\s*$/, "")
    .trim();
  console.log(output);
}

async function handleFileMode(options: CliOptions) {
  if (!options.filePath) {
    throw new Error("Missing --file path.");
  }

  const inputPath = path.resolve(options.filePath);
  const outputPath = path.resolve(options.outPath ?? options.filePath);
  const css = await fs.readFile(inputPath, "utf8");
  const { css: convertedCss, records } = await processCss(css, options, inputPath, outputPath);

  if (options.json) {
    console.log(JSON.stringify(records, null, 2));
    return;
  }

  if (options.write || options.outPath) {
    await fs.writeFile(outputPath, convertedCss, "utf8");
    console.log(
      `Converted ${records.length} declaration${records.length === 1 ? "" : "s"} to OKLCH in ${outputPath}.`,
    );
    return;
  }

  console.log(convertedCss);
}

async function main() {
  const { options, rawEntries } = parseArgs(Bun.argv.slice(2));

  if (options.filePath) {
    await handleFileMode(options);
    return;
  }

  await handleTokenMode(options, rawEntries);
}

try {
  await main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
