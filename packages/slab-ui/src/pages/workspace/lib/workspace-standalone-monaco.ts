import type * as Monaco from "monaco-editor"
import { SyncDescriptor } from "@codingame/monaco-vscode-api/vscode/vs/platform/instantiation/common/descriptors"
import { IMarkdownRendererService } from "@codingame/monaco-vscode-api/vscode/vs/platform/markdown/browser/markdownRenderer.service"
import { MarkdownRendererService } from "@codingame/monaco-vscode-api/vscode/vs/platform/markdown/browser/markdownRenderer"

export function getStandaloneMonacoEditorOverrides() {
  return {
    [IMarkdownRendererService.toString()]: new SyncDescriptor(MarkdownRendererService, [], true),
  } satisfies Monaco.editor.IEditorOverrideServices
}
