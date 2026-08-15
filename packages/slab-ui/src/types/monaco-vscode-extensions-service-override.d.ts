import type { IEditorOverrideServices } from "@codingame/monaco-vscode-api/vscode/vs/editor/standalone/browser/standaloneServices"

export type ExtensionServiceOverridesOptions = {
  enableWorkerExtensionHost?: boolean
  iframeAlternateDomain?: string
}

export default function getServiceOverride(
  options?: ExtensionServiceOverridesOptions,
): IEditorOverrideServices
