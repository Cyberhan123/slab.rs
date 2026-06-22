import { afterAll, beforeAll, describe, expect, it } from "vitest"

import {
  collectBrowserFailureMessages,
  createDesktopWebDriverEnvironment,
  eventually,
  isWorkspaceDesktopFailure,
  startDesktopWebDriver,
  stopDesktopWebDriver,
  type DesktopWebDriverEnvironment,
} from "../support/tauri-webdriver"

describe.sequential("workspace desktop e2e", () => {
  let env: DesktopWebDriverEnvironment | undefined
  let browser: WebdriverIO.Browser

  beforeAll(async () => {
    env = await createDesktopWebDriverEnvironment()
    browser = await startDesktopWebDriver(env)
  }, 900_000)

  afterAll(async () => {
    await stopDesktopWebDriver(env)
  })

  it("renders the VS Code explorer and connects the server terminal without CSP or file-service errors", async () => {
    await waitForSelector('[data-testid="sidebar-link-workspace"]')
    await (await browser.$('[data-testid="sidebar-link-workspace"]')).click()

    await waitForSelector('[data-testid="workspace-vscode-explorer"]')
    await eventually("VS Code explorer renders seeded files", async () => {
      const explorerText = await (await browser.$('[data-testid="workspace-vscode-explorer"]')).getText()
      return explorerText.includes("README.md") && explorerText.includes("src") ? true : null
    }, 120_000)

    await waitForSelector('[data-testid="workspace-console-toggle"]')
    await (await browser.$('[data-testid="workspace-console-toggle"]')).click()
    await waitForSelector('[data-testid="workspace-terminal"] .xterm')
    await waitForSelector('[data-testid="workspace-terminal"] .xterm-helper-textarea')
    await eventually("desktop workspace terminal websocket opens", async () => {
      const state = await (await browser.$('[data-testid="workspace-terminal"]')).getAttribute("data-connection-state")
      return state === "open" ? true : null
    }, 120_000)
    await (await browser.$('[data-testid="workspace-terminal"] .xterm')).click()
    await browser.execute(() => {
      document
        .querySelector<HTMLElement>('[data-testid="workspace-terminal"] .xterm-helper-textarea')
        ?.focus()
    })

    const marker = `desktop-terminal-${Date.now()}`
    const command = process.platform === "win32"
      ? `Write-Output ${marker}`
      : `printf '${marker}\\n'`
    await browser.keys(command)
    await browser.keys("Enter")

    await eventually("desktop workspace terminal prints sentinel", async () => {
      const rowsText = await (await browser.$('[data-testid="workspace-terminal"] .xterm-rows')).getText()
      return rowsText.includes(marker) ? true : null
    }, 120_000)

    const failures = (await collectBrowserFailureMessages(browser)).filter(isWorkspaceDesktopFailure)
    expect(failures).toEqual([])
  }, 240_000)

  async function waitForSelector(selector: string) {
    const element = await browser.$(selector)
    await element.waitForExist({ timeout: 120_000 })
    await element.waitForDisplayed({ timeout: 120_000 })
    return element
  }
})
