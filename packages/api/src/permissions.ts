export const SLAB_API_PERMISSIONS = {
  modelsRead: "models:read",
  ffmpegConvert: "ffmpeg:convert",
  audioTranscribe: "audio:transcribe",
  subtitleRender: "subtitle:render",
  chatComplete: "chat:complete",
  tasksRead: "tasks:read",
  tasksCancel: "tasks:cancel",
} as const;

export type SlabApiPermission =
  (typeof SLAB_API_PERMISSIONS)[keyof typeof SLAB_API_PERMISSIONS];

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
