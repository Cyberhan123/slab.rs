import type * as Monaco from "monaco-editor"

let setupPromise: Promise<void> | null = null

export function setupShikiMonaco(monaco: typeof Monaco) {
  setupPromise ??= Promise.all([
    import("@shikijs/monaco"),
    import("shiki/core"),
    import("@shikijs/engine-javascript"),
    import("@shikijs/themes/light-plus"),
    import("@shikijs/themes/dark-plus"),
    import("@shikijs/langs/bash"),
    import("@shikijs/langs/c"),
    import("@shikijs/langs/cpp"),
    import("@shikijs/langs/css"),
    import("@shikijs/langs/go"),
    import("@shikijs/langs/html"),
    import("@shikijs/langs/java"),
    import("@shikijs/langs/javascript"),
    import("@shikijs/langs/json"),
    import("@shikijs/langs/less"),
    import("@shikijs/langs/markdown"),
    import("@shikijs/langs/powershell"),
    import("@shikijs/langs/python"),
    import("@shikijs/langs/rust"),
    import("@shikijs/langs/scss"),
    import("@shikijs/langs/shell"),
    import("@shikijs/langs/sql"),
    import("@shikijs/langs/toml"),
    import("@shikijs/langs/tsx"),
    import("@shikijs/langs/typescript"),
    import("@shikijs/langs/xml"),
    import("@shikijs/langs/yaml"),
  ]).then(async ([
    { shikiToMonaco },
    { createHighlighterCore },
    { createJavaScriptRegexEngine },
    lightPlus,
    darkPlus,
    ...langs
  ]) => {
    const highlighter = await createHighlighterCore({
      engine: createJavaScriptRegexEngine(),
      themes: [lightPlus.default, darkPlus.default],
      langs: langs.flatMap((lang) => lang.default),
    })

    shikiToMonaco(highlighter, monaco)
    monaco.editor.setTheme("light-plus")
  })

  return setupPromise
}
