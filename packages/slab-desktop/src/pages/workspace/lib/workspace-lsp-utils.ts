const SUPPORTED_WORKSPACE_LSP_LANGUAGES = new Set([
  "typescript",
  "javascript",
  "typescriptreact",
  "javascriptreact",
  "json",
  "css",
  "less",
  "scss",
  "html",
  "python",
  "c",
  "cpp",
  "go",
  "rust",
])

export function supportsWorkspaceLsp(language: string) {
  return SUPPORTED_WORKSPACE_LSP_LANGUAGES.has(language)
}

export function workspaceLspModelPath(workspaceRoot: string, relativePath: string) {
  const path = relativePath.replace(/\\/g, "/").replace(/^\/+/, "")
  const root = normalizeWorkspacePath(workspaceRoot)
  const absolutePath = `${root}/${path}`
  const prefixedPath = absolutePath.startsWith("/") ? absolutePath : `/${absolutePath}`

  return `file://${encodeURI(prefixedPath)}`
}

export function workspaceLspRelativePathFromUri(
  workspaceRoot: string,
  uriString: string,
) {
  let pathname = uriString
  try {
    const url = new URL(uriString)
    if (url.protocol !== "file:") {
      return null
    }
    pathname = url.hostname ? `/${url.hostname}${url.pathname}` : url.pathname
  } catch {
    // Monaco can also hand back path-like strings in tests and internal flows.
  }

  const rootPath = normalizeWorkspacePath(workspaceRoot)
  const absolutePath = normalizeWorkspacePath(decodeURIComponent(pathname))
  const normalizedRoot = rootPath.endsWith("/") ? rootPath : `${rootPath}/`

  if (absolutePath === rootPath) {
    return ""
  }

  if (!absolutePath.startsWith(normalizedRoot)) {
    return null
  }

  return absolutePath.slice(normalizedRoot.length)
}

function normalizeWorkspacePath(path: string) {
  let normalized = path.replace(/\\/g, "/")
  if (/^\/[A-Za-z]:/.test(normalized)) {
    normalized = normalized.slice(1)
  }
  normalized = normalized.replace(/^([A-Za-z]):/, (_, driveLetter: string) => `${driveLetter.toLowerCase()}:`)
  return normalized.replace(/\/+$/, "")
}
