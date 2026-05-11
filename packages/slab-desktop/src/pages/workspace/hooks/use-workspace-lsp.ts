import { useCallback, useEffect, useRef, useState } from "react"
import type { OnMount } from "@monaco-editor/react"

import {
  startWorkspaceLspSession,
  supportsWorkspaceLsp,
  workspaceLspModelUri,
  type WorkspaceLspSession,
} from "../lib/workspace-lsp"

type WorkspaceLspOptions = {
  language: string
  relativePath: string | null
  workspaceRoot: string | null
}

export function useWorkspaceLsp({
  language,
  relativePath,
  workspaceRoot,
}: WorkspaceLspOptions) {
  const editorRef = useRef<Parameters<OnMount>[0] | null>(null)
  const monacoRef = useRef<Parameters<OnMount>[1] | null>(null)
  const sessionRef = useRef<WorkspaceLspSession | null>(null)
  const startGenerationRef = useRef(0)
  const [editorMountVersion, setEditorMountVersion] = useState(0)

  useEffect(() => {
    const generation = startGenerationRef.current + 1
    startGenerationRef.current = generation
    const previousSession = sessionRef.current
    sessionRef.current = null

    void previousSession?.dispose()

    if (!relativePath || !workspaceRoot || !supportsWorkspaceLsp(language)) {
      return
    }

    const editor = editorRef.current
    const monaco = monacoRef.current
    if (!editor || !monaco) {
      return
    }

    let cancelled = false
    const startTimer = window.setTimeout(() => {
      void (async () => {
        if (cancelled || generation !== startGenerationRef.current) {
          return
        }

        const modelUri = workspaceLspModelUri(monaco, workspaceRoot, relativePath)
        const currentModel = editor.getModel()
        if (!currentModel || currentModel.uri.toString() !== modelUri.toString()) {
          return
        }

        const session = await startWorkspaceLspSession({
          language,
          monaco,
          model: currentModel,
          workspaceRoot,
        })
        if (!session) {
          return
        }
        if (cancelled || generation !== startGenerationRef.current) {
          void session.dispose()
          return
        }

        sessionRef.current = session
      })()
    }, 0)

    return () => {
      cancelled = true
      window.clearTimeout(startTimer)
      if (generation === startGenerationRef.current) {
        const currentSession = sessionRef.current
        sessionRef.current = null
        void currentSession?.dispose()
      }
    }
  }, [editorMountVersion, language, relativePath, workspaceRoot])

  const handleEditorMount = useCallback<OnMount>((editor, monaco) => {
    editorRef.current = editor
    monacoRef.current = monaco
    setEditorMountVersion((version) => version + 1)
  }, [])

  return { handleEditorMount }
}
