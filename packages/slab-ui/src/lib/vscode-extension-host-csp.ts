const TAURI_IPC_LOCALHOST = "http://ipc.localhost"

const cspMetaPattern =
  /<meta\s+[^>]*http-equiv=["']Content-Security-Policy["'][^>]*content=(["'])([\s\S]*?)\1[^>]*>/i

export function allowTauriIpcLocalhostInVscodeExtensionHostCsp(html: string): string {
  if (!html.includes("Content-Security-Policy") || !html.includes("connect-src")) {
    return html
  }

  return html.replace(cspMetaPattern, (meta, quote: string, content: string) => {
    if (content.includes(TAURI_IPC_LOCALHOST)) {
      return meta
    }

    const nextContent = content.replace(
      /(connect-src\s+)([^;]*)(;?)/i,
      (_directive, prefix: string, values: string, suffix: string) => {
        const separator = values.trimEnd().length > 0 ? " " : ""
        return `${prefix}${values.trimEnd()}${separator}${TAURI_IPC_LOCALHOST}${suffix}`
      },
    )

    return meta.replace(`${quote}${content}${quote}`, `${quote}${nextContent}${quote}`)
  })
}
