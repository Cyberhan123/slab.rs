import loader from "@monaco-editor/loader"
import * as monaco from "@codingame/monaco-vscode-editor-api"
import type * as Monaco from "monaco-editor"

let configured = false

export function configureWorkspaceMonacoLoader() {
  if (configured) {
    return
  }

  loader.config({ monaco: monaco as unknown as typeof Monaco })
  configured = true
}
