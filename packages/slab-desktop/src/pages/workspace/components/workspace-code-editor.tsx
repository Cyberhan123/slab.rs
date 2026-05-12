import { useEffect, useRef, useState } from "react"
import { createEditor, createModelReference } from "@codingame/monaco-editor-wrapper"
import * as Monaco from "monaco-editor"

import { ensureWorkspaceLspServices } from "../lib/workspace-lsp"

type WorkspaceCodeEditorProps = {
  filePath: string
  language: string
  onChange: (value: string) => void
  onMount?: (
    editor: Monaco.editor.IStandaloneCodeEditor,
    monaco: typeof Monaco,
  ) => void
  options: Monaco.editor.IStandaloneEditorConstructionOptions
  theme: string
  value: string
}

export function WorkspaceCodeEditor({
  filePath,
  language,
  onChange,
  onMount,
  options,
  theme,
  value,
}: WorkspaceCodeEditorProps) {
  const applyingExternalValueRef = useRef(false)
  const containerRef = useRef<HTMLDivElement | null>(null)
  const editorRef = useRef<Monaco.editor.IStandaloneCodeEditor | null>(null)
  const modelRef = useRef<Monaco.editor.ITextModel | null>(null)
  const modelReferenceRef = useRef<{ dispose(): void } | null>(null)
  const onChangeRef = useRef(onChange)
  const onMountRef = useRef(onMount)
  const optionsRef = useRef(options)
  const valueRef = useRef(value)
  const [servicesReady, setServicesReady] = useState(false)

  useEffect(() => {
    onChangeRef.current = onChange
  }, [onChange])

  useEffect(() => {
    onMountRef.current = onMount
  }, [onMount])

  useEffect(() => {
    optionsRef.current = options
    editorRef.current?.updateOptions(options)
  }, [options])

  useEffect(() => {
    valueRef.current = value
  }, [value])

  useEffect(() => {
    let cancelled = false

    void (async () => {
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
  }, [])

  useEffect(() => {
    if (!servicesReady) {
      return
    }

    try {
      Monaco.editor.setTheme(theme)
    } catch (error) {
      console.warn("failed to apply workspace editor theme", error)
      try {
        Monaco.editor.setTheme(theme.toLowerCase().includes("dark") ? "vs-dark" : "vs")
      } catch (fallbackError) {
        console.warn("failed to apply fallback workspace editor theme", fallbackError)
      }
    }
  }, [servicesReady, theme])

  useEffect(() => {
    if (!servicesReady || !containerRef.current || editorRef.current) {
      return
    }

    const initialOptions = optionsRef.current
    const editor = createEditor(containerRef.current, {
      ...initialOptions,
      automaticLayout: initialOptions.automaticLayout ?? true,
      language: undefined,
      model: null,
      theme: undefined,
      value: undefined,
    })
    const contentDisposable = editor.onDidChangeModelContent(() => {
      if (applyingExternalValueRef.current) {
        return
      }

      onChangeRef.current(editor.getValue())
    })

    editorRef.current = editor

    return () => {
      contentDisposable.dispose()
      editor.dispose()
      editorRef.current = null
    }
  }, [servicesReady])

  useEffect(() => {
    const editor = editorRef.current
    if (!servicesReady || !editor) {
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
    }
  }, [filePath, language, servicesReady])

  useEffect(() => {
    const model = modelRef.current
    if (!model || model.uri.toString() !== Monaco.Uri.parse(filePath).toString() || model.getValue() === value) {
      return
    }

    applyingExternalValueRef.current = true
    model.setValue(value)
    applyingExternalValueRef.current = false
  }, [filePath, value])

  if (!servicesReady) {
    return <div className="h-full w-full" />
  }

  return <div ref={containerRef} className="h-full w-full" />
}
