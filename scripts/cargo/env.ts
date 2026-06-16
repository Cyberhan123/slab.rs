import { existsSync } from "node:fs";

export function cargoEnv(options: { disableTauriExternalBins?: boolean; rustWarningsAsErrors?: boolean } = {}) {
  const env = { ...process.env };
  if (options.disableTauriExternalBins) {
    env.TAURI_CONFIG = tauriConfigWithDisabledExternalBins(env.TAURI_CONFIG);
  }
  if (process.platform === "darwin") {
    appendRustflags(env, "-C link-arg=-lc++");
    for (const libDir of ["/opt/homebrew/lib", "/usr/local/lib"]) {
      if (existsSync(libDir)) {
        appendRustflags(env, `-L native=${libDir}`);
      }
    }
  }
  if (options.rustWarningsAsErrors) {
    appendRustflags(env, "-D warnings");
  }
  return env;
}

function appendRustflags(env: NodeJS.ProcessEnv, flag: string) {
  env.RUSTFLAGS = env.RUSTFLAGS ? `${env.RUSTFLAGS} ${flag}` : flag;
}

function tauriConfigWithDisabledExternalBins(existing: string | undefined) {
  const config = existing ? parseJsonObject(existing, "TAURI_CONFIG") : {};
  const bundle = isPlainObject(config.bundle) ? config.bundle : {};
  config.bundle = { ...bundle, externalBin: null };
  return JSON.stringify(config);
}

function parseJsonObject(value: string, name: string) {
  let parsed: unknown;
  try {
    parsed = JSON.parse(value);
  } catch (error) {
    throw new Error(`${name} must contain valid JSON: ${String(error)}`, { cause: error });
  }
  if (!isPlainObject(parsed)) {
    throw new Error(`${name} must be a JSON object.`);
  }
  return { ...parsed };
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
