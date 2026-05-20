import {
  SimpleTerminalBackend,
  SimpleTerminalProcess,
  type ITerminalBackend,
  type ITerminalChildProcess,
} from "@codingame/monaco-vscode-terminal-service-override"
import * as vscode from "vscode"

import { workspaceTerminalSession } from "@/lib/workspace-bridge"

let nextTerminalProcessId = 1

export class SlabTerminalBackend extends SimpleTerminalBackend {
  override getDefaultSystemShell: ITerminalBackend["getDefaultSystemShell"] = async () =>
    navigator.userAgent.includes("Windows") ? "powershell.exe" : "sh"

  override createProcess: ITerminalBackend["createProcess"] = async (
    _shellLaunchConfig,
    cwd,
    cols,
    rows,
  ): Promise<ITerminalChildProcess> => new SlabTerminalProcess(cwd, cols, rows)
}

export const slabTerminalBackend = new SlabTerminalBackend()

class SlabTerminalProcess extends SimpleTerminalProcess {
  private readonly dataEmitter: vscode.EventEmitter<string>
  private readonly exitEmitter = new vscode.EventEmitter<number | undefined>()
  private readonly textDecoder = new TextDecoder()
  private socket: WebSocket | null = null

  override readonly onProcessExit = this.exitEmitter.event

  constructor(cwd: string, private cols: number, private rows: number) {
    const id = nextTerminalProcessId++
    const dataEmitter = new vscode.EventEmitter<string>()
    super(id, id, cwd, dataEmitter.event)
    this.dataEmitter = dataEmitter
  }

  async start() {
    try {
      const { url } = await workspaceTerminalSession()
      return await new Promise<undefined>((resolve, reject) => {
        const socket = new WebSocket(url)
        this.socket = socket
        socket.binaryType = "arraybuffer"

        socket.addEventListener(
          "open",
          () => {
            this.resize(this.cols, this.rows)
            resolve(undefined)
          },
          { once: true },
        )
        socket.addEventListener("message", (event) => {
          void this.writeSocketData(event.data)
        })
        socket.addEventListener(
          "error",
          () => reject(new Error("workspace terminal websocket failed")),
          { once: true },
        )
        socket.addEventListener("close", () => {
          this.socket = null
          this.exitEmitter.fire(undefined)
        })
      })
    } catch (error) {
      return {
        message: error instanceof Error ? error.message : "failed to start workspace terminal",
      }
    }
  }

  override shutdown(_immediate: boolean) {
    this.socket?.close()
    this.socket = null
  }

  override input(data: string) {
    this.sendControl({ type: "input", data })
  }

  override resize(cols: number, rows: number) {
    this.cols = cols
    this.rows = rows
    this.sendControl({ type: "resize", cols, rows })
  }

  override clearBuffer() {}

  override sendSignal(signal: string) {
    if (signal === "SIGINT") {
      this.input("\x03")
    }
  }

  private async writeSocketData(data: unknown) {
    if (data instanceof ArrayBuffer) {
      this.dataEmitter.fire(this.textDecoder.decode(new Uint8Array(data), { stream: true }))
      return
    }
    if (data instanceof Blob) {
      const buffer = await data.arrayBuffer()
      this.dataEmitter.fire(this.textDecoder.decode(new Uint8Array(buffer), { stream: true }))
      return
    }
    this.dataEmitter.fire(String(data))
  }

  private sendControl(message: { type: "input"; data: string } | { type: "resize"; cols: number; rows: number }) {
    if (this.socket?.readyState === WebSocket.OPEN) {
      this.socket.send(JSON.stringify(message))
    }
  }
}
