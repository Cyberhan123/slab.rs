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

export type WorkspaceLspOpenFileOptions = {
  endColumn?: number
  endLineNumber?: number
  startColumn?: number
  startLineNumber?: number
}

export type WorkspaceLspDefinitionTarget = WorkspaceLspOpenFileOptions & {
  relativePath: string
}

export type WorkspaceLspPosition = {
  column: number
  lineNumber: number
}

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

export function workspaceLspDefinitionTargetFromResult(
  workspaceRoot: string,
  definitions: unknown,
): WorkspaceLspDefinitionTarget | null {
  if (!definitions) {
    return null
  }

  const definitionEntries = Array.isArray(definitions) ? definitions : [definitions]
  for (const definition of definitionEntries) {
    const target = workspaceLspDefinitionTargetFromEntry(workspaceRoot, definition)
    if (target) {
      return target
    }
  }

  return null
}

export function workspaceLspImportSpecifierPositionForTarget(
  lineText: string,
  target: WorkspaceLspDefinitionTarget,
): WorkspaceLspPosition | null {
  if (!target.startLineNumber || !target.startColumn) {
    return null
  }

  const searchStart = Math.max(0, (target.endColumn ?? target.startColumn) - 1)
  const fromMatch = /\bfrom\s*(['"])/.exec(lineText.slice(searchStart))
  if (!fromMatch || fromMatch.index === undefined) {
    return null
  }

  const quoteIndex = searchStart + fromMatch.index + fromMatch[0].length - 1
  return {
    column: quoteIndex + 2,
    lineNumber: target.startLineNumber,
  }
}

export function workspaceVscodeDirtyCloseTarget(
  workspaceRoot: string,
  input: unknown,
  isDirty: (resource: string) => boolean,
) {
  const resource = workspaceVscodeResourceStringFromEditorInput(input)
  if (!resource) {
    return null
  }

  const relativePath = workspaceLspRelativePathFromUri(workspaceRoot, resource)
  if (!relativePath) {
    return null
  }

  return isDirty(resource) ? relativePath : null
}

export function workspaceVscodeResourceStringFromEditorInput(input: unknown) {
  if (!input || typeof input !== "object") {
    return null
  }

  const record = input as Record<string, unknown>
  const resource = record.resource
  if (resource && typeof resource === "object" && "toString" in resource) {
    return resource.toString()
  }

  const toUntyped = record.toUntyped
  if (typeof toUntyped !== "function") {
    return null
  }

  const untyped = toUntyped.call(input)
  if (!untyped || typeof untyped !== "object") {
    return null
  }

  const untypedResource = (untyped as Record<string, unknown>).resource
  if (untypedResource && typeof untypedResource === "object" && "toString" in untypedResource) {
    return untypedResource.toString()
  }

  return null
}

function workspaceLspDefinitionTargetFromEntry(
  workspaceRoot: string,
  definition: unknown,
): WorkspaceLspDefinitionTarget | null {
  if (!definition || typeof definition !== "object") {
    return null
  }

  const record = definition as Record<string, unknown>
  const uriString = uriStringFromValue(record.targetUri ?? record.uri)
  if (!uriString) {
    return null
  }

  const relativePath = workspaceLspRelativePathFromUri(workspaceRoot, uriString)
  if (relativePath === null) {
    return null
  }

  return {
    relativePath,
    ...selectionFromRange(record.targetSelectionRange ?? record.range ?? record.targetRange),
  }
}

function selectionFromRange(range: unknown): WorkspaceLspOpenFileOptions {
  if (!range || typeof range !== "object") {
    return {}
  }

  const record = range as Record<string, unknown>
  const start = lineCharacterPosition(record.start)
  if (!start) {
    return {}
  }

  const end = lineCharacterPosition(record.end)
  return {
    endColumn: end?.column ?? start.column,
    endLineNumber: end?.lineNumber ?? start.lineNumber,
    startColumn: start.column,
    startLineNumber: start.lineNumber,
  }
}

function lineCharacterPosition(position: unknown) {
  if (!position || typeof position !== "object") {
    return null
  }

  const record = position as Record<string, unknown>
  if (typeof record.line !== "number" || typeof record.character !== "number") {
    return null
  }

  return {
    column: record.character + 1,
    lineNumber: record.line + 1,
  }
}

function uriStringFromValue(value: unknown) {
  if (typeof value === "string") {
    return value
  }

  if (!value) {
    return null
  }

  return String(value)
}

function normalizeWorkspacePath(path: string) {
  let normalized = path.replace(/\\/g, "/")
  if (normalized.startsWith("//?/UNC/")) {
    normalized = `//${normalized.slice("//?/UNC/".length)}`
  } else if (normalized.startsWith("//?/")) {
    normalized = normalized.slice("//?/".length)
  }
  if (/^\/[A-Za-z]:/.test(normalized)) {
    normalized = normalized.slice(1)
  }
  normalized = normalized.replace(/^([A-Za-z]):/, (_, driveLetter: string) => `${driveLetter.toLowerCase()}:`)
  return normalized.replace(/\/+$/, "")
}
