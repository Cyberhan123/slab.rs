import type * as Monaco from "monaco-editor"

export type WorkspaceThemeMode = "light" | "dark"

export const SLAB_MONACO_THEME_LIGHT = "slab-light"
export const SLAB_MONACO_THEME_DARK = "slab-dark"

const fallbackColors = {
  dark: {
    background: "#151d21",
    brandGold: "#f1c27d",
    brandTeal: "#4cc7ba",
    border: "#2a3841",
    foreground: "#f3f7f7",
    muted: "#93a4ad",
    primary: "#4cc7ba",
    selection: "#26343c",
    surfaceSoft: "#1e2a30",
  },
  light: {
    background: "#ffffff",
    brandGold: "#855300",
    brandTeal: "#0d9488",
    border: "#e2e8ee",
    foreground: "#191c1e",
    muted: "#6d7a77",
    primary: "#0d9488",
    selection: "#e0e3e5",
    surfaceSoft: "#f2f4f6",
  },
} as const

export function slabMonacoThemeId(mode: WorkspaceThemeMode) {
  return mode === "dark" ? SLAB_MONACO_THEME_DARK : SLAB_MONACO_THEME_LIGHT
}

export function getWorkspaceThemeMode(): WorkspaceThemeMode {
  if (typeof document === "undefined") {
    return "light"
  }

  return document.documentElement.classList.contains("dark") ? "dark" : "light"
}

export function buildSlabMonacoTheme(mode: WorkspaceThemeMode): Monaco.editor.IStandaloneThemeData {
  const fallback = fallbackColors[mode]
  const css = (name: string, defaultColor: string) => readCssColor(name, defaultColor)
  const token = (name: string, defaultColor: string) => css(name, defaultColor).replace(/^#/, "")

  return {
    base: mode === "dark" ? "vs-dark" : "vs",
    inherit: true,
    rules: [
      { token: "comment", foreground: token("--muted-foreground", fallback.muted), fontStyle: "italic" },
      { token: "keyword", foreground: token("--brand-teal", fallback.brandTeal) },
      { token: "string", foreground: token("--brand-gold", fallback.brandGold) },
      { token: "number", foreground: token("--chart-2", fallback.primary) },
      { token: "type", foreground: token("--chart-4", fallback.brandGold) },
      { token: "function", foreground: token("--primary", fallback.primary) },
    ],
    colors: {
      "editor.background": css("--surface-1", fallback.background),
      "editor.foreground": css("--foreground", fallback.foreground),
      "editor.lineHighlightBackground": css("--surface-soft", fallback.surfaceSoft),
      "editor.selectionBackground": css("--surface-selected", fallback.selection),
      "editorCursor.foreground": css("--brand-teal", fallback.brandTeal),
      "editorGutter.background": css("--surface-1", fallback.background),
      "editorIndentGuide.background1": css("--border", fallback.border),
      "editorLineNumber.foreground": css("--muted-foreground", fallback.muted),
      "editorWhitespace.foreground": css("--border", fallback.border),
      "scrollbarSlider.activeBackground": css("--muted-foreground", fallback.muted),
      "scrollbarSlider.background": css("--border", fallback.border),
      "scrollbarSlider.hoverBackground": css("--surface-selected", fallback.selection),
    },
  }
}

export function registerSlabMonacoTheme(monaco: typeof Monaco, mode: WorkspaceThemeMode) {
  monaco.editor.defineTheme(slabMonacoThemeId(mode), buildSlabMonacoTheme(mode))
}

export function applySlabMonacoTheme(monaco: typeof Monaco, mode: WorkspaceThemeMode) {
  const fallbackThemeId = mode === "dark" ? "vs-dark" : "vs"
  let themeId = slabMonacoThemeId(mode)

  try {
    registerSlabMonacoTheme(monaco, mode)
  } catch {
    themeId = fallbackThemeId
  }

  try {
    monaco.editor.setTheme(themeId)
  } catch {
    if (themeId !== fallbackThemeId) {
      monaco.editor.setTheme(fallbackThemeId)
      return fallbackThemeId
    }
  }

  return themeId
}

function readCssColor(name: string, fallback: string) {
  if (typeof document === "undefined") {
    return fallback
  }

  const rawValue = getComputedStyle(document.documentElement).getPropertyValue(name).trim()
  return normalizeCssColor(rawValue, fallback)
}

function normalizeCssColor(value: string, fallback: string) {
  if (!value) {
    return fallback
  }

  const canvas = document.createElement("canvas")
  const context = canvas.getContext("2d")
  if (!context) {
    return fallback
  }

  context.fillStyle = fallback
  context.fillStyle = value
  const normalized = context.fillStyle

  if (normalized.startsWith("#")) {
    return normalized
  }

  const rgbMatch = normalized.match(/^rgba?\((\d+),\s*(\d+),\s*(\d+)/)
  if (!rgbMatch) {
    return fallback
  }

  return [rgbMatch[1], rgbMatch[2], rgbMatch[3]]
    .map((part) => Number(part).toString(16).padStart(2, "0"))
    .join("")
    .padStart(7, "#")
}
