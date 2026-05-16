import { useEffect, useRef, useState } from "react"
import * as Monaco from "monaco-editor"

import { ensureWorkspaceLspServices } from "../lib/workspace-lsp"
import { languageForFile } from "../lib/workspace-page-utils"

type WorkspaceDiffEditorProps = {
  diffText: string
  filePath: string
  theme: string
}

type ParsedDiff = {
  original: string
  modified: string
}

function parseUnifiedDiff(diffText: string): ParsedDiff {
  const lines = diffText.split("\n")
  const originalChunks: string[][] = []
  const modifiedChunks: string[][] = []
  let currentOriginal: string[] = []
  let currentModified: string[] = []
  let inHunk = false

  for (const line of lines) {
    if (line.startsWith("@@")) {
      if (inHunk) {
        originalChunks.push(currentOriginal)
        modifiedChunks.push(currentModified)
        currentOriginal = []
        currentModified = []
      }
      inHunk = true
      continue
    }

    if (
      !inHunk ||
      line.startsWith("---") ||
      line.startsWith("+++") ||
      line.startsWith("diff ") ||
      line.startsWith("index ")
    ) {
      continue
    }

    if (line.startsWith("+")) {
      currentModified.push(line.slice(1))
    } else if (line.startsWith("-")) {
      currentOriginal.push(line.slice(1))
    } else {
      const content = line.startsWith(" ") ? line.slice(1) : line
      currentOriginal.push(content)
      currentModified.push(content)
    }
  }

  if (currentOriginal.length > 0 || currentModified.length > 0) {
    originalChunks.push(currentOriginal)
    modifiedChunks.push(currentModified)
  }

  const separator = ["", "// ...", ""]
  const original = originalChunks
    .flatMap((chunk, i) => (i > 0 ? separator.concat(chunk) : chunk))
    .join("\n")
  const modified = modifiedChunks
    .flatMap((chunk, i) => (i > 0 ? separator.concat(chunk) : chunk))
    .join("\n")

  return { original, modified }
}

export function WorkspaceDiffEditor({ diffText, filePath, theme }: WorkspaceDiffEditorProps) {
  const containerRef = useRef<HTMLDivElement | null>(null)
  const editorRef = useRef<Monaco.editor.IStandaloneDiffEditor | null>(null)
  const [servicesReady, setServicesReady] = useState(false)

  useEffect(() => {
    let cancelled = false

    void (async () => {
      await ensureWorkspaceLspServices()
      if (cancelled) {
        return
      }
      setServicesReady(true)
    })().catch((error) => {
      console.error("failed to initialize workspace diff editor", error)
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
      console.warn("failed to apply workspace diff editor theme", error)
    }
  }, [servicesReady, theme])

  useEffect(() => {
    if (!servicesReady || !containerRef.current || editorRef.current) {
      return
    }

    const editor = Monaco.editor.createDiffEditor(containerRef.current, {
      automaticLayout: true,
      readOnly: true,
      renderSideBySide: true,
      minimap: { enabled: false },
      scrollBeyondLastLine: false,
      fontSize: 13,
      wordWrap: "on",
    })

    editorRef.current = editor

    return () => {
      editor.dispose()
      editorRef.current = null
    }
  }, [servicesReady])

  useEffect(() => {
    const editor = editorRef.current
    if (!editor || !servicesReady) {
      return
    }

    const language = languageForFile(filePath.split("/").pop() ?? filePath)
    const { original, modified } = parseUnifiedDiff(diffText)

    const originalModel = Monaco.editor.createModel(original, language)
    const modifiedModel = Monaco.editor.createModel(modified, language)

    editor.setModel({ original: originalModel, modified: modifiedModel })

    return () => {
      editor.setModel(null)
      originalModel.dispose()
      modifiedModel.dispose()
    }
  }, [diffText, filePath, servicesReady])

  if (!servicesReady) {
    return <div className="h-full w-full" />
  }

  return <div ref={containerRef} className="h-full w-full" />
}
