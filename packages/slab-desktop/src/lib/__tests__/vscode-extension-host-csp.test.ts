import { describe, expect, it } from "vitest"

import { allowTauriIpcLocalhostInVscodeExtensionHostCsp } from "../vscode-extension-host-csp"

describe("VS Code extension host CSP", () => {
  it("adds the Tauri IPC localhost origin to connect-src", () => {
    const html = `<html><head>
      <meta http-equiv="Content-Security-Policy" content="
        default-src 'none';
        connect-src 'self' data: extension-file: https: wss: http://localhost:* http://127.0.0.1:* ws://localhost:* ws://127.0.0.1:*;"/>
    </head></html>`

    const patched = allowTauriIpcLocalhostInVscodeExtensionHostCsp(html)

    expect(patched).toContain("connect-src 'self' data:")
    expect(patched).toContain("http://ipc.localhost")
  })

  it("does not duplicate the Tauri IPC localhost origin", () => {
    const html = `<meta http-equiv="Content-Security-Policy" content="connect-src 'self' http://ipc.localhost;">`

    const patched = allowTauriIpcLocalhostInVscodeExtensionHostCsp(html)

    expect(patched.match(/http:\/\/ipc\.localhost/g)).toHaveLength(1)
  })
})
