export const SLAB_API_PERMISSIONS = {
  modelsRead: "models:read",
  modelsLoad: "models:load",
  ffmpegConvert: "ffmpeg:convert",
  audioTranscribe: "audio:transcribe",
  subtitleRender: "subtitle:render",
  chatComplete: "chat:complete",
  tasksRead: "tasks:read",
  tasksCancel: "tasks:cancel",
} as const;

export type SlabApiPermission =
  (typeof SLAB_API_PERMISSIONS)[keyof typeof SLAB_API_PERMISSIONS];

export type SlabApiPermissionSeverity = "low" | "medium" | "high";

export type SlabApiPermissionLabel = {
  title: string;
  description: string;
  severity: SlabApiPermissionSeverity;
};

/**
 * Human-readable metadata for every permission on the plugin Slab API surface.
 * Used by the import-time permission preview and the runtime first-reject
 * authorization prompt so users can understand what a plugin is asking for
 * instead of trusting an opaque `chat:complete` string.
 */
export const SLAB_API_PERMISSION_LABELS: Record<SlabApiPermission, SlabApiPermissionLabel> = {
  [SLAB_API_PERMISSIONS.modelsRead]: {
    title: "Read models",
    description: "List available models and read their metadata.",
    severity: "low",
  },
  [SLAB_API_PERMISSIONS.modelsLoad]: {
    title: "Load models",
    description: "Load (and download) models into the local runtime, which uses disk, memory, and compute.",
    severity: "high",
  },
  [SLAB_API_PERMISSIONS.ffmpegConvert]: {
    title: "Run FFmpeg conversions",
    description: "Convert and process media files through the FFmpeg tool runtime.",
    severity: "medium",
  },
  [SLAB_API_PERMISSIONS.audioTranscribe]: {
    title: "Transcribe audio",
    description: "Run audio transcription, which can consume significant compute for long files.",
    severity: "medium",
  },
  [SLAB_API_PERMISSIONS.subtitleRender]: {
    title: "Render subtitles",
    description: "Render and write subtitle assets to disk.",
    severity: "medium",
  },
  [SLAB_API_PERMISSIONS.chatComplete]: {
    title: "Run chat completions",
    description: "Send prompts to the local model and read generated responses.",
    severity: "high",
  },
  [SLAB_API_PERMISSIONS.tasksRead]: {
    title: "Read tasks",
    description: "Inspect background task status and results.",
    severity: "low",
  },
  [SLAB_API_PERMISSIONS.tasksCancel]: {
    title: "Cancel tasks",
    description: "Cancel running background tasks, including model downloads.",
    severity: "medium",
  },
};

const UNKNOWN_PERMISSION_LABEL: SlabApiPermissionLabel = {
  title: "Unknown permission",
  description:
    "This permission is not part of the recognized plugin Slab API surface. Grant it only if you trust the plugin author.",
  severity: "high",
};

export function isKnownSlabApiPermission(
  permission: string,
): permission is SlabApiPermission {
  return Object.hasOwn(SLAB_API_PERMISSION_LABELS, permission);
}

export function describeSlabApiPermission(permission: string): SlabApiPermissionLabel {
  return isKnownSlabApiPermission(permission)
    ? SLAB_API_PERMISSION_LABELS[permission]
    : { ...UNKNOWN_PERMISSION_LABEL, title: `Unknown permission: ${permission}` };
}

export function requiredSlabApiPermission(
  method: string,
  path: string,
): SlabApiPermission | null {
  const normalizedMethod = method.toUpperCase();
  const normalizedPath = path.split("?").at(0) ?? path;

  switch (normalizedMethod) {
    case "GET":
      if (pathMatches(normalizedPath, "/v1/models")) {
        return SLAB_API_PERMISSIONS.modelsRead;
      }
      if (pathMatches(normalizedPath, "/v1/tasks")) {
        return SLAB_API_PERMISSIONS.tasksRead;
      }
      return null;
    case "POST":
      if (normalizedPath === "/v1/models/load") {
        return SLAB_API_PERMISSIONS.modelsLoad;
      }
      if (normalizedPath === "/v1/ffmpeg/convert") {
        return SLAB_API_PERMISSIONS.ffmpegConvert;
      }
      if (normalizedPath === "/v1/audio/transcriptions") {
        return SLAB_API_PERMISSIONS.audioTranscribe;
      }
      if (normalizedPath === "/v1/subtitles/render") {
        return SLAB_API_PERMISSIONS.subtitleRender;
      }
      if (normalizedPath === "/v1/chat/completions") {
        return SLAB_API_PERMISSIONS.chatComplete;
      }
      if (normalizedPath.startsWith("/v1/tasks/") && normalizedPath.endsWith("/cancel")) {
        return SLAB_API_PERMISSIONS.tasksCancel;
      }
      return null;
    default:
      return null;
  }
}

export function assertSlabPluginApiSurface(method: string, path: string): SlabApiPermission {
  const requiredPermission = requiredSlabApiPermission(method, path);
  if (requiredPermission) {
    return requiredPermission;
  }

  throw new Error(
    `Plugin API request ${method.toUpperCase()} ${path} is not part of the allowed plugin API surface.`,
  );
}

function pathMatches(path: string, base: string): boolean {
  return path === base || path.startsWith(`${base}/`);
}
