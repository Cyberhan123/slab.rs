/**
 * Default-extension readiness for the embedded VS Code editor.
 *
 * The editor pulls in only the subset of `@codingame/monaco-vscode-*-default-extension`
 * packages that this workspace actually renders (theme icons, language grammars, and a
 * few language-feature extensions). `whenWorkspaceExtensionsReady` is the single
 * aggregation point the service layer awaits during initialization so the editor does
 * not present content before its grammar/theme contributions are wired.
 *
 * The search-result extension import replaces the side effect that
 * `@codingame/monaco-editor-wrapper/features/search` used to provide: it registers the
 * renderer for the workspace search result editor alongside the search service override
 * registered in {@link "./workspace-services.ts"}.
 */

import "@codingame/monaco-vscode-search-result-default-extension"
import { whenReady as cppExtensionReady } from "@codingame/monaco-vscode-cpp-default-extension"
import { whenReady as cssExtensionReady } from "@codingame/monaco-vscode-css-default-extension"
import { whenReady as goExtensionReady } from "@codingame/monaco-vscode-go-default-extension"
import { whenReady as htmlExtensionReady } from "@codingame/monaco-vscode-html-default-extension"
import { whenReady as javascriptExtensionReady } from "@codingame/monaco-vscode-javascript-default-extension"
import { whenReady as jsonExtensionReady } from "@codingame/monaco-vscode-json-default-extension"
import { whenReady as markdownBasicsExtensionReady } from "@codingame/monaco-vscode-markdown-basics-default-extension"
import { whenReady as markdownLanguageFeaturesExtensionReady } from "@codingame/monaco-vscode-markdown-language-features-default-extension"
import { whenReady as sqlExtensionReady } from "@codingame/monaco-vscode-sql-default-extension"
import { whenReady as themeDefaultsExtensionReady } from "@codingame/monaco-vscode-theme-defaults-default-extension"
import { whenReady as setiThemeExtensionReady } from "@codingame/monaco-vscode-theme-seti-default-extension"
import { whenReady as typescriptBasicsExtensionReady } from "@codingame/monaco-vscode-typescript-basics-default-extension"
import { whenReady as xmlExtensionReady } from "@codingame/monaco-vscode-xml-default-extension"
import { whenReady as yamlExtensionReady } from "@codingame/monaco-vscode-yaml-default-extension"
import { whenReady as emmetExtensionReady } from "@codingame/monaco-vscode-emmet-default-extension"
import { whenReady as dockerExtensionReady } from "@codingame/monaco-vscode-docker-default-extension"
import { whenReady as dotenvExtensionReady } from "@codingame/monaco-vscode-dotenv-default-extension"

/**
 * Resolves once the workspace's default extensions have finished registering.
 * Uses `Promise.allSettled` so a single failing extension never blocks editor init.
 */
export async function whenWorkspaceExtensionsReady(): Promise<void> {
  await Promise.allSettled([
    themeDefaultsExtensionReady(),
    setiThemeExtensionReady(),
    typescriptBasicsExtensionReady(),
    javascriptExtensionReady(),
    jsonExtensionReady(),
    cssExtensionReady(),
    htmlExtensionReady(),
    markdownBasicsExtensionReady(),
    markdownLanguageFeaturesExtensionReady(),
    yamlExtensionReady(),
    emmetExtensionReady(),
    dockerExtensionReady(),
    dotenvExtensionReady(),
    cppExtensionReady(),
    goExtensionReady(),
    sqlExtensionReady(),
    xmlExtensionReady(),
  ])
}
