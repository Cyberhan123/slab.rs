import { loader } from "@monaco-editor/react"
import * as monaco from "monaco-editor"
import type * as Monaco from "monaco-editor"

let configured = false

export function configureWorkspaceMonacoLoader() {
  if (configured) {
    return
  }

  loader.config({ monaco: monaco as unknown as typeof Monaco })
  configured = true
}
