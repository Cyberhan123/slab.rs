import { apiClient } from "@slab/api"

export type ServerHealth = {
  healthy: boolean
  raw: unknown
}

/**
 * Probe the slab-server health endpoint. Transport failures resolve to
 * `healthy: false` instead of throwing — health is a status probe, and the
 * caller renders either way.
 */
export async function getServerHealth(): Promise<ServerHealth> {
  try {
    const result = await apiClient.GET("/health")
    if (!result || result.response.status >= 400) {
      return { healthy: false, raw: null }
    }
    return { healthy: true, raw: result.data }
  } catch {
    return { healthy: false, raw: null }
  }
}
