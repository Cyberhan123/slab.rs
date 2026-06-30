export function normalizeWorkspaceArtifactPath(path: string | undefined | null) {
  const trimmed = path?.trim()
  if (!trimmed || isAbsoluteFsPath(trimmed) || /^[a-zA-Z]:/.test(trimmed)) {
    return null
  }

  const parts = trimmed
    .replaceAll("\\", "/")
    .split("/")
    .filter((part) => part && part !== ".")

  if (parts.length === 0 || parts.some((part) => part === "..")) {
    return null
  }

  return parts.join("/")
}

function isAbsoluteFsPath(path: string) {
  return /^[a-zA-Z]:[\\/]/.test(path) || path.startsWith("/") || path.startsWith("\\")
}
