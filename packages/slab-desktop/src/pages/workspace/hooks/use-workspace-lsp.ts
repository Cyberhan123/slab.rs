import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react"
import type * as Monaco from "monaco-editor"

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
  onOpenFile: (relativePath: string, options?: { revealInTree?: boolean }) => Promise<unknown>
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
  const shouldInitializeServices = Boolean(workspaceRoot)
  const shouldUseLsp = Boolean(workspaceRoot && supportsWorkspaceLsp(language))
  const editorRef = useRef<Monaco.editor.IStandaloneCodeEditor | null>(null)
  const monacoRef = useRef<typeof Monaco | null>(null)
  const sessionRef = useRef<WorkspaceLspSession | null>(null)
  const sessionKeyRef = useRef<string | null>(null)
  const startGenerationRef = useRef(0)
  const [editorModelVersion, setEditorModelVersion] = useState(0)
  const [editorMountVersion, setEditorMountVersion] = useState(0)
  const [servicesState, setServicesState] = useState<WorkspaceLspServicesState>(() =>
    initialServicesState(shouldInitializeServices),
  )

  useLayoutEffect(() => {
    setWorkspaceLspFileServiceRoot(workspaceRoot)
    return () => {
      setWorkspaceLspFileServiceRoot(null)
    }
  }, [workspaceRoot])

  useEffect(() => {
    let cancelled = false

    if (!shouldInitializeServices) {
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
  }, [language, shouldInitializeServices, workspaceRoot])

  const openFileInEditor = useCallback(
    async (nextRelativePath: string, options?: WorkspaceLspOpenFileOptions) => {
      if (nextRelativePath !== relativePath) {
        await onOpenFile(nextRelativePath, { revealInTree: false })
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
    },
    [onOpenFile, relativePath, workspaceRoot],
  )

  useLayoutEffect(() => {
    setWorkspaceLspOpenFile(openFileInEditor)

    return () => {
      setWorkspaceLspOpenFile(null)
    }
  }, [openFileInEditor])

  useEffect(() => {
    if (servicesState !== "ready" || !workspaceRoot || !shouldUseLsp) {
      return
    }

    const editor = editorRef.current
    if (!editor) {
      return
    }

    const mouseDownDisposable = editor.onMouseDown((event) => {
      const session = sessionRef.current
      const model = editor.getModel()
      const position = event.target.position
      if (!session || !model || !position || !event.event.leftButton || (!event.event.ctrlKey && !event.event.metaKey)) {
        return
      }

      event.event.preventDefault()
      event.event.stopPropagation()
      void (async () => {
        const target = await session.definitionTarget(
          model,
          position,
        )
        if (!target) {
          return
        }

        await openFileInEditor(target.relativePath, target)
      })().catch((error) => {
        console.debug("workspace LSP definition click failed", {
          position,
          uri: model.uri.toString(),
          error,
        })
      })
    })

    return () => {
      mouseDownDisposable.dispose()
    }
  }, [editorMountVersion, openFileInEditor, servicesState, shouldUseLsp, workspaceRoot])

  useEffect(() => {
    const nextSessionKey =
      servicesState === "ready" && workspaceRoot && shouldUseLsp ? `${workspaceRoot}\0${language}` : null
    if (sessionRef.current && sessionKeyRef.current !== nextSessionKey) {
      const previousSession = sessionRef.current
      sessionRef.current = null
      sessionKeyRef.current = null
      startGenerationRef.current += 1
      void previousSession.dispose()
    }

    if (!nextSessionKey || !workspaceRoot || sessionRef.current || !relativePath) {
      return
    }
    const sessionWorkspaceRoot = workspaceRoot

    const editor = editorRef.current
    const monaco = monacoRef.current
    if (!editor || !monaco) {
      return
    }

    const generation = startGenerationRef.current + 1
    startGenerationRef.current = generation
    let cancelled = false
    const startTimer = window.setTimeout(() => {
      void (async () => {
        if (cancelled || generation !== startGenerationRef.current) {
          return
        }

        const modelUri = workspaceLspModelUri(monaco, sessionWorkspaceRoot, relativePath)
        const currentModel = editor.getModel()
        if (!currentModel || currentModel.uri.toString() !== modelUri.toString()) {
          return
        }

        const session = await startWorkspaceLspSession({
          language,
          monaco,
          model: currentModel,
          workspaceRoot: sessionWorkspaceRoot,
        })
        if (!session) {
          return
        }
        if (cancelled || generation !== startGenerationRef.current || sessionRef.current) {
          void session.dispose()
          return
        }

        sessionRef.current = session
        sessionKeyRef.current = nextSessionKey
      })()
    }, 0)

    return () => {
      cancelled = true
      window.clearTimeout(startTimer)
    }
  }, [editorModelVersion, editorMountVersion, language, relativePath, servicesState, shouldUseLsp, workspaceRoot])

  useEffect(() => {
    if (servicesState !== "ready" || !relativePath || !workspaceRoot || !shouldUseLsp) {
      return
    }

    const editor = editorRef.current
    const monaco = monacoRef.current
    const session = sessionRef.current
    if (!editor || !monaco || !session) {
      return
    }

    let cancelled = false
    void (async () => {
      const modelUri = workspaceLspModelUri(monaco, workspaceRoot, relativePath)
      await waitForEditorModel(editor, modelUri.toString())
      const currentModel = editor.getModel()
      if (cancelled || !currentModel || currentModel.uri.toString() !== modelUri.toString()) {
        return
      }

      await session.registerModel(currentModel)
    })().catch((error) => {
      console.debug("workspace LSP document registration failed", { language, relativePath, workspaceRoot, error })
    })

    return () => {
      cancelled = true
    }
  }, [editorModelVersion, language, relativePath, servicesState, shouldUseLsp, workspaceRoot])

  const handleEditorMount = useCallback((editor: Monaco.editor.IStandaloneCodeEditor, monaco: typeof Monaco) => {
    const mountedEditorChanged = editorRef.current !== editor
    editorRef.current = editor
    monacoRef.current = monaco
    if (mountedEditorChanged) {
      setEditorMountVersion((version) => version + 1)
    }
    setEditorModelVersion((version) => version + 1)
  }, [])

  useEffect(() => {
    return () => {
      const currentSession = sessionRef.current
      sessionRef.current = null
      sessionKeyRef.current = null
      startGenerationRef.current += 1
      void currentSession?.dispose()
    }
  }, [])

  return {
    handleEditorMount,
    servicesPending: shouldInitializeServices && servicesState !== "ready" && servicesState !== "failed",
    servicesReady: servicesState === "ready",
  }
}

function initialServicesState(shouldInitializeServices: boolean): WorkspaceLspServicesState {
  if (!shouldInitializeServices) {
    return "idle"
  }

  return workspaceLspServicesReady() ? "ready" : "pending"
}

function waitForEditorModel(
  editor: Monaco.editor.IStandaloneCodeEditor,
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
  editor: Monaco.editor.IStandaloneCodeEditor,
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
