import { normalizeWorkspaceArtifactPath } from "@/lib/workspace-artifact-path"
import type { AgentSurfaceInput } from "@/store/useAgentSurfaceStore"

type A2uToolRule = {
  kind: AgentSurfaceInput["type"]
  riskLevel: "allow" | "ask" | "sandbox"
  toolName: string
}

export const A2U_DISPATCH_TABLE = [
  { toolName: "workspace.open", kind: "workspace", riskLevel: "allow" },
  { toolName: "review.show", kind: "review", riskLevel: "allow" },
  { toolName: "image.edit", kind: "image", riskLevel: "allow" },
  { toolName: "plugin.launch", kind: "plugin", riskLevel: "ask" },
  { toolName: "hub.browse", kind: "hub", riskLevel: "allow" },
] as const satisfies readonly A2uToolRule[]

export type A2uDispatchResult = {
  riskLevel: A2uToolRule["riskLevel"]
  surface: AgentSurfaceInput
}

export function dispatchA2uToolCall(
  toolName: string,
  rawArguments: string
): A2uDispatchResult | null {
  const rule = A2U_DISPATCH_TABLE.find((item) => item.toolName === toolName)
  if (!rule) {
    return null
  }

  const args = parseToolArguments(rawArguments)
  switch (rule.kind) {
    case "workspace":
      return {
        riskLevel: rule.riskLevel,
        surface: {
          type: "workspace",
          payload: {
            revealPath: normalizeWorkspaceArtifactPath(
              firstStringValue(args, "revealPath", "path", "file", "relativePath")
            ) ?? undefined,
          },
        },
      }
    case "review":
      return {
        riskLevel: rule.riskLevel,
        surface: {
          type: "review",
          payload: {
            diff: firstStringValue(args, "diff", "patch"),
            path: normalizeWorkspaceArtifactPath(
              firstStringValue(args, "path", "file", "relativePath")
            ) ?? undefined,
          },
        },
      }
    case "image":
      return {
        riskLevel: rule.riskLevel,
        surface: {
          type: "image",
          payload: {
            prompt: firstStringValue(args, "prompt", "description"),
          },
        },
      }
    case "plugin":
      return {
        riskLevel: rule.riskLevel,
        surface: {
          type: "plugin",
          payload: {
            payload: args.payload,
            pluginId: firstStringValue(args, "pluginId", "plugin_id"),
            surface: firstStringValue(args, "surface", "view"),
          },
        },
      }
    case "hub":
      return {
        riskLevel: rule.riskLevel,
        surface: {
          type: "hub",
          payload: {},
        },
      }
    default:
      return null
  }
}

function parseToolArguments(rawArguments: string): Record<string, unknown> {
  if (!rawArguments.trim()) {
    return {}
  }

  try {
    const parsed: unknown = JSON.parse(rawArguments)
    return typeof parsed === "object" && parsed !== null && !Array.isArray(parsed)
      ? parsed as Record<string, unknown>
      : {}
  } catch {
    return {}
  }
}

function firstStringValue(args: Record<string, unknown>, ...keys: string[]) {
  for (const key of keys) {
    const value = args[key]
    if (typeof value === "string" && value.trim()) {
      return value.trim()
    }
  }

  return undefined
}
