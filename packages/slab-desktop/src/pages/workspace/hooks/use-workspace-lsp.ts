import { useCallback, useEffect, useRef, useState } from "react"
import type { OnMount } from "@monaco-editor/react"

import {
  ensureWorkspaceLspServices,
  setWorkspaceLspFileServiceRoot,
  setWorkspaceLspOpenFile,
  startWorkspaceLspSession,
  supportsWorkspaceLsp,
  workspaceLspServicesReady,
  workspaceLspModelUri,
  type WorkspaceLspSession,
  type WorkspaceLspOpenFileOptions,
} from "../lib/workspace-lsp"

type WorkspaceLspOptions = {
  language: string
  onOpenFile: (relativePath: string) => Promise<void>
  relativePath: string | null
  workspaceRoot: string | null
}

type WorkspaceLspServicesState = "idle" | "pending" | "ready" | "failed"

export function useWorkspaceLsp({
  language,
  onOpenFile,
  relativePath,
  workspaceRoot,
}: WorkspaceLspOptions) {
  const shouldUseLsp = Boolean(workspaceRoot && supportsWorkspaceLsp(language))
  const editorRef = useRef<Parameters<OnMount>[0] | null>(null)
  const monacoRef = useRef<Parameters<OnMount>[1] | null>(null)
  const sessionRef = useRef<WorkspaceLspSession | null>(null)
  const startGenerationRef = useRef(0)
  const [editorMountVersion, setEditorMountVersion] = useState(0)
  const [servicesState, setServicesState] = useState<WorkspaceLspServicesState>(() =>
    initialServicesState(shouldUseLsp),
  )

  useEffect(() => {
    setWorkspaceLspFileServiceRoot(workspaceRoot)
    return () => {
      setWorkspaceLspFileServiceRoot(null)
    }
  }, [workspaceRoot])

  useEffect(() => {
    let cancelled = false

    if (!shouldUseLsp) {
      setServicesState("idle")
      return
    }

    if (workspaceLspServicesReady()) {
      setServicesState("ready")
      return
    }

    setServicesState("pending")
    void ensureWorkspaceLspServices()
      .then(() => {
        if (!cancelled) {
          setServicesState("ready")
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setServicesState("failed")
        }
        console.debug("workspace LSP services unavailable", { language, workspaceRoot, error })
      })

    return () => {
      cancelled = true
    }
  }, [language, shouldUseLsp, workspaceRoot])

  useEffect(() => {
    setWorkspaceLspOpenFile(async (nextRelativePath, options) => {
      if (nextRelativePath !== relativePath) {
        await onOpenFile(nextRelativePath)
      }

      const editor = editorRef.current
      const monaco = monacoRef.current
      if (!editor || !monaco || !workspaceRoot) {
        return undefined
      }

      const modelUri = workspaceLspModelUri(monaco, workspaceRoot, nextRelativePath)
      await waitForEditorModel(editor, modelUri.toString())
      if (editor.getModel()?.uri.toString() !== modelUri.toString()) {
        return undefined
      }

      applySelection(editor, options)
      editor.focus()
      return editor
    })

    return () => {
      setWorkspaceLspOpenFile(null)
    }
  }, [onOpenFile, relativePath, workspaceRoot])

  useEffect(() => {
    const generation = startGenerationRef.current + 1
    startGenerationRef.current = generation
    const previousSession = sessionRef.current
    sessionRef.current = null

    void previousSession?.dispose()

    if (servicesState !== "ready" || !relativePath || !workspaceRoot || !supportsWorkspaceLsp(language)) {
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
  }, [editorMountVersion, language, relativePath, servicesState, workspaceRoot])

  const handleEditorMount = useCallback<OnMount>((editor, monaco) => {
    editorRef.current = editor
    monacoRef.current = monaco
    setEditorMountVersion((version) => version + 1)
  }, [])

  return {
    handleEditorMount,
    servicesPending: shouldUseLsp && servicesState !== "ready" && servicesState !== "failed",
    servicesReady: servicesState === "ready",
  }
}

function initialServicesState(shouldUseLsp: boolean): WorkspaceLspServicesState {
  if (!shouldUseLsp) {
    return "idle"
  }

  return workspaceLspServicesReady() ? "ready" : "pending"
}

function waitForEditorModel(
  editor: Parameters<OnMount>[0],
  expectedUri: string,
) {
  if (editor.getModel()?.uri.toString() === expectedUri) {
    return Promise.resolve()
  }

  return new Promise<void>((resolve) => {
    let frames = 0
    const check = () => {
      frames += 1
      if (editor.getModel()?.uri.toString() === expectedUri || frames >= 12) {
        resolve()
        return
      }
      window.requestAnimationFrame(check)
    }
    window.requestAnimationFrame(check)
  })
}

function applySelection(
  editor: Parameters<OnMount>[0],
  options: WorkspaceLspOpenFileOptions | undefined,
) {
  if (!options?.startLineNumber || !options.startColumn) {
    return
  }

  editor.setSelection({
    endColumn: options.endColumn ?? options.startColumn,
    endLineNumber: options.endLineNumber ?? options.startLineNumber,
    startColumn: options.startColumn,
    startLineNumber: options.startLineNumber,
  })
  editor.revealPositionInCenter({
    column: options.startColumn,
    lineNumber: options.startLineNumber,
  })
}
