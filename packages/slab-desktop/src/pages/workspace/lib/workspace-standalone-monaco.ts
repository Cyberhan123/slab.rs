import type * as Monaco from "monaco-editor"

let editorOverridesPromise: Promise<Monaco.editor.IEditorOverrideServices> | null = null

export function getStandaloneMonacoEditorOverrides() {
  editorOverridesPromise ??= Promise.all([
    import("@codingame/monaco-vscode-api/vscode/vs/platform/instantiation/common/descriptors"),
    import("@codingame/monaco-vscode-api/vscode/vs/platform/markdown/browser/markdownRenderer.service"),
    import("@codingame/monaco-vscode-api/vscode/vs/platform/markdown/browser/markdownRenderer"),
  ]).then(([{ SyncDescriptor }, { IMarkdownRendererService }, { MarkdownRendererService }]) => ({
    [IMarkdownRendererService.toString()]: new SyncDescriptor(MarkdownRendererService, [], true),
  }))

  return editorOverridesPromise
}
