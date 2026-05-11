import { loader } from "@monaco-editor/react"
import * as monaco from "monaco-editor"
import type * as Monaco from "monaco-editor"

// Configure MonacoEnvironment.getWorkerUrl synchronously at module load time,
// before any editor instance is created. Monaco's getWorkerUrl takes priority
// over each language mode's createWorker callback, so all language labels must
// be mapped to their correct worker file, not just the fallback editor worker.
;(globalThis.MonacoEnvironment as Record<string, unknown>) ??= {}
;(globalThis.MonacoEnvironment as { getWorkerUrl?: (workerId: string, label: string) => string }).getWorkerUrl ??=
  ((_workerId: string, label: string): string => {
    switch (label) {
      case "typescript":
      case "javascript":
      case "typescriptreact":
      case "javascriptreact":
        return new URL(
          "monaco-editor/esm/vs/language/typescript/ts.worker.js",
          import.meta.url,
        ).toString()
      case "json":
        return new URL(
          "monaco-editor/esm/vs/language/json/json.worker.js",
          import.meta.url,
        ).toString()
      case "css":
      case "less":
      case "scss":
        return new URL(
          "monaco-editor/esm/vs/language/css/css.worker.js",
          import.meta.url,
        ).toString()
      case "html":
      case "handlebars":
      case "razor":
        return new URL(
          "monaco-editor/esm/vs/language/html/html.worker.js",
          import.meta.url,
        ).toString()
      default:
        return new URL(
          "@codingame/monaco-vscode-editor-api/esm/vs/editor/editor.worker.js",
          import.meta.url,
        ).toString()
    }
  })

let configured = false

export function configureWorkspaceMonacoLoader() {
  if (configured) {
    return
  }

  loader.config({ monaco: monaco as unknown as typeof Monaco })

  // The workspace uses a real TypeScript language server via LSP (WebSocket to the backend).
  // Disable semantic validation in the built-in Monaco TypeScript service to prevent false
  // errors such as "Cannot use JSX" and "Cannot find module '@slab/...'" that arise because
  // the built-in service does not read the workspace tsconfig.json. The LSP handles type
  // checking, module resolution, and JSX correctly via the workspace tsconfig.
  //
  // @codingame/monaco-vscode-api marks monaco.languages.typescript as deprecated at the type
  // level, but the underlying runtime object is still present (the TypeScript language
  // contribution is explicitly loaded via the ts.worker.js mapping above).
  type MonacoTsDefaults = {
    setDiagnosticsOptions(opts: { noSemanticValidation?: boolean }): void
  }
  type MonacoTsLanguage = {
    typescript: { typescriptDefaults: MonacoTsDefaults; javascriptDefaults: MonacoTsDefaults }
  }
  const tsLang = (monaco.languages as unknown as MonacoTsLanguage).typescript
  tsLang.typescriptDefaults.setDiagnosticsOptions({ noSemanticValidation: true })
  tsLang.javascriptDefaults.setDiagnosticsOptions({ noSemanticValidation: true })

  configured = true
}
