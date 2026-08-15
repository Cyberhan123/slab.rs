import JSZip from "jszip";

/**
 * The subset of a plugin manifest shown during the import permission review.
 * Mirrors `PluginManifest`/`PluginPermissionsManifest` from `slab-types` but only
 * the fields the user needs to make an install decision.
 */
export type PluginManifestPreview = {
  id: string | null;
  name: string | null;
  version: string | null;
  permissions: {
    slabApi: string[];
    filesRead: string[];
    filesWrite: string[];
    networkMode: string | null;
    networkHosts: string[];
    agent: string[];
    lsp: string[];
  };
  parseError: string | null;
};

const EMPTY_PREVIEW: PluginManifestPreview = {
  id: null,
  name: null,
  version: null,
  permissions: {
    slabApi: [],
    filesRead: [],
    filesWrite: [],
    networkMode: null,
    networkHosts: [],
    agent: [],
    lsp: [],
  },
  parseError: null,
};

function toStringArray(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((entry): entry is string => typeof entry === "string") : [];
}

/**
 * Reads `plugin.json` out of a `.plugin.slab` (ZIP) pack entirely client-side so the
 * user can review the permissions a plugin is requesting *before* it is uploaded and
 * installed. Returns `null` when the pack cannot be read as a zip or contains no
 * manifest; the caller falls back to importing without a preview. Nothing is
 * executed and no entry is extracted beyond `plugin.json`.
 */
export async function parsePluginPackManifest(file: File): Promise<PluginManifestPreview | null> {
  let zip: JSZip;
  try {
    zip = await JSZip.loadAsync(file);
  } catch {
    return null;
  }

  const candidates: string[] = [];
  zip.forEach((relativePath, entry) => {
    if (!entry.dir && relativePath.split("/").pop() === "plugin.json") {
      candidates.push(relativePath);
    }
  });
  if (candidates.length === 0) {
    return null;
  }
  // A pack may nest the plugin dir (e.g. `archive-root/plugin-id/plugin.json`).
  // Prefer the shallowest manifest, matching the backend `locate_plugin_root` intent.
  candidates.sort((a, b) => a.split("/").length - b.split("/").length);
  const manifestFile = zip.file(candidates[0]);
  if (!manifestFile) {
    return null;
  }

  let text: string;
  try {
    text = await manifestFile.async("text");
  } catch {
    return null;
  }

  let manifest: Record<string, unknown>;
  try {
    manifest = JSON.parse(text) as Record<string, unknown>;
  } catch (error) {
    return {
      ...EMPTY_PREVIEW,
      parseError: error instanceof Error ? error.message : "invalid plugin.json",
    };
  }

  const permissions = (manifest.permissions ?? {}) as Record<string, unknown>;
  const network = (permissions.network ?? {}) as Record<string, unknown>;

  return {
    id: typeof manifest.id === "string" ? manifest.id : null,
    name: typeof manifest.name === "string" ? manifest.name : null,
    version: typeof manifest.version === "string" ? manifest.version : null,
    permissions: {
      slabApi: toStringArray(permissions.slabApi),
      filesRead: toStringArray((permissions.files as Record<string, unknown> | undefined)?.read),
      filesWrite: toStringArray((permissions.files as Record<string, unknown> | undefined)?.write),
      networkMode: typeof network.mode === "string" ? network.mode : null,
      networkHosts: toStringArray(network.allowHosts),
      agent: toStringArray(permissions.agent),
      lsp: toStringArray(permissions.lsp),
    },
    parseError: null,
  };
}
