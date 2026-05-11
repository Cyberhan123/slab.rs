import { useEffect, useRef, useState } from "react"
import Editor, { monaco } from "@codingame/monaco-editor-react"
import type * as Monaco from "monaco-editor"

import { ensureWorkspaceLspServices } from "../lib/workspace-lsp"

type WorkspaceCodeEditorProps = {
  filePath: string
  language: string
  onBeforeMount?: (monaco: typeof Monaco) => void | Promise<void>
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
  onBeforeMount,
  onChange,
  onMount,
  options,
  theme,
  value,
}: WorkspaceCodeEditorProps) {
  const onBeforeMountRef = useRef(onBeforeMount)
  const onMountRef = useRef(onMount)
  const [editorInstance, setEditorInstance] = useState<Monaco.editor.IStandaloneCodeEditor | null>(null)
  const [servicesReady, setServicesReady] = useState(false)

  useEffect(() => {
    onBeforeMountRef.current = onBeforeMount
  }, [onBeforeMount])

  useEffect(() => {
    onMountRef.current = onMount
  }, [onMount])

  useEffect(() => {
    let cancelled = false

    void (async () => {
      await ensureWorkspaceLspServices()
      await onBeforeMountRef.current?.(monaco)
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

    monaco.editor.setTheme(theme)
  }, [servicesReady, theme])

  useEffect(() => {
    if (!editorInstance) {
      return
    }

    onMountRef.current?.(editorInstance, monaco)
  }, [editorInstance])

  if (!servicesReady) {
    return <div className="h-full w-full" />
  }

  return (
    <div className="h-full w-full">
      <Editor
        ref={setEditorInstance}
        fileUri={filePath}
        height="100%"
        onChange={(nextValue) => {
          onChange(nextValue)
        }}
        options={options}
        programmingLanguage={language}
        value={value}
      />
    </div>
  )
}