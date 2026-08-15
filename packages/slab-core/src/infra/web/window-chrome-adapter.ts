import type { WindowChromePort } from "../../ports"

/** No native window chrome on the web; every action is a no-op. */
export const webWindowChrome: WindowChromePort = {
  async minimize() {},
  async toggleMaximize() {},
  async close() {},
  isAvailable() {
    return false
  },
}
