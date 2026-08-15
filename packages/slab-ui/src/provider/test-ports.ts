import type { SlabPorts } from "@slab/core";

/** Partial override set accepted by {@link createTestSlabPorts}. */
export type TestSlabPortsOverrides = Partial<SlabPorts>;

/**
 * Browser-default port set for tests. Pass overrides to stub a specific
 * capability (e.g. `createTestSlabPorts({ fileDialog: fakeFileDialog })`).
 */
export function createTestSlabPorts(
  overrides?: TestSlabPortsOverrides,
): SlabPorts {
  return {
    fileDialog: {
      async pickFolder() {
        return null
      },
      async pickFile() {
        return null
      },
      async pickFiles() {
        return []
      },
    },
    mediaFile: {
      async readFile() {
        throw new Error("not implemented in test ports")
      },
      async writeTempAudio() {
        throw new Error("not implemented in test ports")
      },
      async removeTempAudio() {},
    },
    imageSrc: {
      resolve(pathOrUrl) {
        return pathOrUrl
      },
      canResolveLocalPaths() {
        return false
      },
    },
    notifications: {
      error() {},
    },
    platformInfo: {
      desktop: false,
      mobile: false,
    },
    ...overrides,
  }
}
