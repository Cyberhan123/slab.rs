import { useEffect, useRef, useState } from "react"
import * as Monaco from "monaco-editor"
import { clamp } from "lodash-es"

import { getStandaloneMonacoEditorOverrides } from "../lib/workspace-standalone-monaco"
import {
  applySlabMonacoTheme,
  slabMonacoThemeId,
  type WorkspaceThemeMode,
} from "../lib/monaco-theme"

type WorkspaceCodeEditorProps = {
  filePath: string
  language: string
  memoryModel?: boolean
  onChange: (value: string) => void
  onCursorChange?: (cursor: WorkspaceEditorCursor | null) => void
  onMount?: (
    editor: Monaco.editor.IStandaloneCodeEditor,
    monaco: typeof Monaco,
  ) => void
  onProblemsChange?: (problems: WorkspaceEditorProblem[]) => void
  onSelectionChange?: (selection: WorkspaceEditorSelection | null) => void
  options: Monaco.editor.IStandaloneEditorConstructionOptions
  revealTarget?: WorkspaceEditorRevealTarget | null
  themeMode: WorkspaceThemeMode
  value: string
}

export type WorkspaceEditorCursor = {
  column: number
  lineNumber: number
}

export type WorkspaceEditorProblem = Monaco.editor.IMarker

export type WorkspaceEditorSelection = {
  endColumn: number
  endLineNumber: number
  startColumn: number
  startLineNumber: number
  text: string
}

export type WorkspaceEditorRevealTarget = {
  lineNumber: number
  matchStart: number
  matchEnd: number
}

function revealEditorTarget(
  editor: Monaco.editor.IStandaloneCodeEditor,
  model: Monaco.editor.ITextModel,
  target: WorkspaceEditorRevealTarget,
) {
  const maxLineNumber = model.getLineCount()
  const lineNumber = clamp(target.lineNumber, 1, maxLineNumber)
  const lineLength = model.getLineMaxColumn(lineNumber)
  const startColumn = clamp(target.matchStart + 1, 1, lineLength)
  const endColumn = clamp(target.matchEnd + 1, startColumn, lineLength)
  editor.setSelection({
    startLineNumber: lineNumber,
    startColumn,
    endLineNumber: lineNumber,
    endColumn,
  })
  editor.revealLineInCenter(lineNumber)
  editor.focus()
}

export function WorkspaceCodeEditor({
  filePath,
  language,
  memoryModel = false,
  onChange,
  onCursorChange,
  onMount,
  onProblemsChange,
  onSelectionChange,
  options,
  revealTarget,
  themeMode,
  value,
}: WorkspaceCodeEditorProps) {
  const applyingExternalValueRef = useRef(false)
  const containerRef = useRef<HTMLDivElement | null>(null)
  const editorRef = useRef<Monaco.editor.IStandaloneCodeEditor | null>(null)
  const modelRef = useRef<Monaco.editor.ITextModel | null>(null)
  const modelReferenceRef = useRef<{ dispose(): void } | null>(null)
  const onChangeRef = useRef(onChange)
  const onCursorChangeRef = useRef(onCursorChange)
  const onMountRef = useRef(onMount)
  const onProblemsChangeRef = useRef(onProblemsChange)
  const onSelectionChangeRef = useRef(onSelectionChange)
  const optionsRef = useRef(options)
  const revealTargetRef = useRef(revealTarget)
  const themeModeRef = useRef(themeMode)
  const valueRef = useRef(value)
  const [editorReady, setEditorReady] = useState(false)
  const [servicesReady, setServicesReady] = useState(false)

  useEffect(() => {
    onChangeRef.current = onChange
  }, [onChange])

  useEffect(() => {
    onCursorChangeRef.current = onCursorChange
  }, [onCursorChange])

  useEffect(() => {
    onMountRef.current = onMount
  }, [onMount])

  useEffect(() => {
    onProblemsChangeRef.current = onProblemsChange
  }, [onProblemsChange])

  useEffect(() => {
    onSelectionChangeRef.current = onSelectionChange
  }, [onSelectionChange])

  useEffect(() => {
    optionsRef.current = options
    editorRef.current?.updateOptions(options)
  }, [options])

  useEffect(() => {
    revealTargetRef.current = revealTarget
  }, [revealTarget])

  useEffect(() => {
    themeModeRef.current = themeMode
  }, [themeMode])

  useEffect(() => {
    valueRef.current = value
  }, [value])

  useEffect(() => {
    let cancelled = false

    if (memoryModel) {
      setServicesReady(true)
      return () => {
        cancelled = true
      }
    }

    void (async () => {
      const { ensureWorkspaceLspServices } = await import("../lib/workspace-lsp")
      await ensureWorkspaceLspServices()
      if (cancelled) {
        return
      }
      setServicesReady(true)
    })().catch((error) => {
      console.error("failed to initialize workspace editor", error)
    })

    return () => {
      cancelled = true
    }
  }, [memoryModel])

  useEffect(() => {
    if (!servicesReady || (memoryModel && !editorReady)) {
      return
    }

    try {
      applySlabMonacoTheme(Monaco, themeMode)
    } catch (error) {
      console.warn("failed to apply workspace editor theme", error)
      try {
        Monaco.editor.setTheme(themeMode === "dark" ? "vs-dark" : "vs")
      } catch (fallbackError) {
        console.warn("failed to apply fallback workspace editor theme", fallbackError)
      }
    }
  }, [editorReady, memoryModel, servicesReady, themeMode])

  useEffect(() => {
    if (!servicesReady || !containerRef.current || editorRef.current) {
      return
    }

    let disposed = false
    let editor: Monaco.editor.IStandaloneCodeEditor | null = null
    const disposables: Array<{ dispose(): void }> = []

    void (async () => {
      const container = containerRef.current
      if (!container) {
        return
      }

      const initialOptions = optionsRef.current
      const currentThemeMode = themeModeRef.current
      const themeId = slabMonacoThemeId(currentThemeMode)
      applySlabMonacoTheme(Monaco, currentThemeMode)
      const nextEditor = memoryModel
        ? (await import("@codingame/monaco-vscode-api/monaco")).createConfiguredEditor(
            container,
            {
              ...initialOptions,
              automaticLayout: initialOptions.automaticLayout ?? true,
              language: undefined,
              model: null,
              theme: themeId,
              value: undefined,
            },
            await getStandaloneMonacoEditorOverrides(),
          )
        : (await import("@codingame/monaco-editor-wrapper")).createEditor(container, {
            ...initialOptions,
            automaticLayout: initialOptions.automaticLayout ?? true,
            language: undefined,
            model: null,
            theme: undefined,
            value: undefined,
          })

      if (disposed) {
        nextEditor.dispose()
        return
      }

      editor = nextEditor
      disposables.push(
        nextEditor.onDidChangeModelContent(() => {
          if (applyingExternalValueRef.current) {
            return
          }

          onChangeRef.current(nextEditor.getValue())
        }),
        nextEditor.onDidChangeCursorPosition(({ position }: Monaco.editor.ICursorPositionChangedEvent) => {
          onCursorChangeRef.current?.(position)
        }),
        nextEditor.onDidChangeCursorSelection(({ selection }: Monaco.editor.ICursorSelectionChangedEvent) => {
          const model = nextEditor.getModel()
          if (!model || selection.isEmpty()) {
            onSelectionChangeRef.current?.(null)
            return
          }

          onSelectionChangeRef.current?.({
            endColumn: selection.endColumn,
            endLineNumber: selection.endLineNumber,
            startColumn: selection.startColumn,
            startLineNumber: selection.startLineNumber,
            text: model.getValueInRange(selection),
          })
        }),
      )

      editorRef.current = nextEditor
      setEditorReady(true)
    })().catch((error) => {
      console.error("failed to create workspace editor", {
        error,
        memoryModel,
      })
    })

    return () => {
      disposed = true
      disposables.forEach((disposable) => disposable.dispose())
      editor?.dispose()
      if (editorRef.current === editor) {
        editorRef.current = null
      }
      onCursorChangeRef.current?.(null)
      onProblemsChangeRef.current?.([])
      onSelectionChangeRef.current?.(null)
    }
  }, [memoryModel, servicesReady])

  useEffect(() => {
    if (!servicesReady || !editorReady) {
      return
    }

    const disposable = Monaco.editor.onDidChangeMarkers((resources) => {
      const model = modelRef.current
      if (!model || !resources.some((resource) => resource.toString() === model.uri.toString())) {
        return
      }

      onProblemsChangeRef.current?.(Monaco.editor.getModelMarkers({ resource: model.uri }))
    })

    return () => {
      disposable.dispose()
    }
  }, [editorReady, servicesReady])

  useEffect(() => {
    const editor = editorRef.current
    if (!servicesReady || !editorReady || !editor) {
      return
    }

    let cancelled = false
    let modelReference: { dispose(): void } | null = null
    const uri = Monaco.Uri.parse(filePath)
    const uriString = uri.toString()

    void (async () => {
      if (cancelled) {
        return
      }

      if (memoryModel) {
        const existingModel = Monaco.editor.getModel(uri)
        const model = existingModel ?? Monaco.editor.createModel(valueRef.current, language, uri)
        if (model.getLanguageId() !== language) {
          Monaco.editor.setModelLanguage(model, language)
        }
        if (model.getValue() !== valueRef.current) {
          applyingExternalValueRef.current = true
          model.setValue(valueRef.current)
          applyingExternalValueRef.current = false
        }
        editor.setModel(model)
        modelRef.current = model
        onMountRef.current?.(editor, Monaco)
        onCursorChangeRef.current?.(editor.getPosition())
        onProblemsChangeRef.current?.(Monaco.editor.getModelMarkers({ resource: model.uri }))
        if (revealTargetRef.current) {
          revealEditorTarget(editor, model, revealTargetRef.current)
        }
        return
      }

      const { createModelReference } = await import("@codingame/monaco-editor-wrapper")
      const nextModelReference = await createModelReference(uri)
      modelReference = nextModelReference
      if (cancelled) {
        nextModelReference.dispose()
        return
      }

      const model = nextModelReference.object.textEditorModel as unknown as Monaco.editor.ITextModel | null
      if (!model) {
        throw new Error("workspace editor model was not created")
      }
      if (model.getLanguageId() !== language) {
        Monaco.editor.setModelLanguage(model, language)
      }
      if (model.getValue() !== valueRef.current) {
        applyingExternalValueRef.current = true
        model.setValue(valueRef.current)
        applyingExternalValueRef.current = false
      }

      editor.setModel(model)
      modelRef.current = model
      modelReferenceRef.current = nextModelReference
      onMountRef.current?.(editor, Monaco)
      onCursorChangeRef.current?.(editor.getPosition())
      onProblemsChangeRef.current?.(Monaco.editor.getModelMarkers({ resource: model.uri }))
      if (revealTargetRef.current) {
        revealEditorTarget(editor, model, revealTargetRef.current)
      }
    })().catch((error) => {
      console.error("failed to open workspace editor model", { filePath: uriString, error })
      modelReference?.dispose()
    })

    return () => {
      cancelled = true
      if (modelRef.current?.uri.toString() === uriString) {
        editor.setModel(null)
        modelRef.current = null
      }
      if (modelReferenceRef.current === modelReference) {
        modelReferenceRef.current = null
      }
      modelReference?.dispose()
      if (memoryModel) {
        Monaco.editor.getModel(uri)?.dispose()
      }
    }
  }, [editorReady, filePath, language, memoryModel, servicesReady])

  useEffect(() => {
    const model = modelRef.current
    if (!model || model.uri.toString() !== Monaco.Uri.parse(filePath).toString() || model.getValue() === value) {
      return
    }

    applyingExternalValueRef.current = true
    model.setValue(value)
    applyingExternalValueRef.current = false
  }, [filePath, value])

  useEffect(() => {
    const editor = editorRef.current
    const model = modelRef.current
    if (!editor || !model || !revealTarget) {
      return
    }

    revealEditorTarget(editor, model, revealTarget)
  }, [revealTarget])

  if (!servicesReady) {
    return <div className="h-full w-full" />
  }

  return <div ref={containerRef} className="h-full w-full" />
}
