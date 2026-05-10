declare module "@gytx/xterm-local-echo" {
  import type { ITerminalAddon } from "@xterm/xterm"

  export type LocalEchoOptions = {
    enableAutocomplete?: boolean
    enableIncompleteInput?: boolean
    historySize?: number
    maxAutocompleteEntries?: number
  }

  export class LocalEchoAddon implements ITerminalAddon {
    constructor(options?: LocalEchoOptions)
    activate(terminal: Parameters<ITerminalAddon["activate"]>[0]): void
    dispose(): void
    addAutocompleteHandler(fn: Function, ...args: unknown[]): void
  }
}
