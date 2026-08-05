#!/usr/bin/env bun
/**
 * Asserts the `slab-agent` purity invariant (AGENTS.md): the crate must not
 * directly depend on any HTTP/RPC/IO framework or any higher slab layer that
 * would break its "pure control-plane library" boundary:
 *   sqlx, axum, tonic, slab-agent-rollout, slab-app-core, slab-proto
 *
 * Scans the direct `[dependencies]` / `[dev-dependencies]` / `[build-dependencies]`
 * (incl. target-scoped) entries of `crates/slab-agent/Cargo.toml`. Transitive deps
 * pulled in through allowed crates (e.g. slab-types) are out of scope — only a
 * *direct* forbidden dep is a boundary violation. Exits 1 with the offenders on
 * violation, 0 when clean.
 */
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");

const TARGET_TOML = path.join(repoRoot, "crates", "slab-agent", "Cargo.toml");

// The architectural invariant. Extend here if the purity contract grows.
const FORBIDDEN = [
  "sqlx",
  "axum",
  "tonic",
  "slab-agent-rollout",
  "slab-app-core",
  "slab-proto",
];

const DEPENDENCY_SECTION_SUFFIXES = ["dependencies", "dev-dependencies", "build-dependencies"];

function isDependencySection(header: string): boolean {
  // Header text already has the surrounding brackets stripped, e.g.
  // `dependencies`, `dev-dependencies`, `target.'cfg(unix)'.dependencies`.
  return DEPENDENCY_SECTION_SUFFIXES.some((suffix) => header === suffix || header.endsWith(`.${suffix}`));
}

/** Extract the dependency key from a `key = value` manifest line, or null. */
function dependencyKey(line: string): string | null {
  const trimmed = line.trim();
  if (trimmed === "" || trimmed.startsWith("#")) return null;
  const match = trimmed.match(/^"?([A-Za-z0-9_.-]+)"?\s*=/);
  return match ? match[1]! : null;
}

function collectDirectDependencies(toml: string): string[] {
  const keys: string[] = [];
  let inDependencySection = false;
  for (const rawLine of toml.split(/\r?\n/)) {
    const headerMatch = rawLine.match(/^\s*\[([^\]]+)\]\s*$/);
    if (headerMatch) {
      inDependencySection = isDependencySection(headerMatch[1]!.trim());
      continue;
    }
    if (!inDependencySection) continue;
    const key = dependencyKey(rawLine);
    if (key) keys.push(key);
  }
  return keys;
}

try {
  const toml = readFileSync(TARGET_TOML, "utf8");
  const forbiddenSet = new Set(FORBIDDEN);
  const offenders = collectDirectDependencies(toml).filter((dep) => forbiddenSet.has(dep));

  if (offenders.length > 0) {
    console.error(
      `slab-agent purity violation: forbidden direct ${offenders.length === 1 ? "dependency" : "dependencies"} ` +
        `[${offenders.join(", ")}] found in ${path.relative(repoRoot, TARGET_TOML)}. ` +
        `slab-agent must remain a pure control-plane library (no sqlx/axum/tonic or higher slab layers).`,
    );
    process.exit(1);
  }

  console.log(`slab-agent purity OK: no forbidden direct dependencies in ${path.relative(repoRoot, TARGET_TOML)}.`);
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
