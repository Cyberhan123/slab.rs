import { FileSearch, FolderOpen, MessageSquarePlus } from "lucide-react"
import { useState } from "react"

import { Button } from "@slab/components/button"
import { workspaceValidatePath } from "@/lib/workspace-bridge"
import { normalizeWorkspaceArtifactPath } from "@/lib/workspace-artifact-path"
import { cn } from "@/lib/utils"
import { useAgentSurfaceStore } from "@/store/useAgentSurfaceStore"

import type { AssistantArtifactRef } from "../assistant-context"

type AgentActionCardLabels = {
  blockedPath: string
  feedback: string
  open: string
  review: string
  title: string
}

type AgentActionCardProps = {
  artifactRefs: AssistantArtifactRef[]
  className?: string
  labels: AgentActionCardLabels
  onFeedback: (prompt: string) => void
}

export function AgentActionCard({
  artifactRefs,
  className,
  labels,
  onFeedback,
}: AgentActionCardProps) {
  const [validationState, setValidationState] = useState<"blocked" | "idle" | "validating">("idle")
  const primaryArtifact = artifactRefs.find((artifact) => artifact.path.trim())
  const safePath = primaryArtifact ? normalizeWorkspaceArtifactPath(primaryArtifact.path) : null
  const disabled = !safePath || validationState !== "idle"

  if (!primaryArtifact) {
    return null
  }

  const dispatchWorkspaceSurface = async (surface: "review" | "workspace") => {
    if (!safePath || validationState !== "idle") {
      return
    }

    setValidationState("validating")
    try {
      const result = await workspaceValidatePath(safePath)
      if (surface === "workspace") {
        useAgentSurfaceStore.getState().setPendingSurface({
          type: "workspace",
          payload: {
            revealPath: result.relativePath,
          },
        })
      } else {
        useAgentSurfaceStore.getState().setPendingSurface({
          type: "review",
          payload: {
            path: result.relativePath,
          },
        })
      }
    } catch {
      setValidationState("blocked")
      return
    }

    setValidationState("idle")
  }

  const pathBlocked = !safePath || validationState === "blocked"
  const title = pathBlocked ? labels.blockedPath : labels.title

  return (
    <div
      className={cn(
        "flex max-w-[min(100%,42rem)] flex-wrap items-center gap-2 rounded-2xl border border-border/60 bg-[var(--surface-soft)] px-3 py-2",
        className
      )}
      data-testid="agent-action-card"
    >
      <span className="min-w-0 flex-1 truncate text-xs font-medium text-muted-foreground">
        {title}
      </span>
      <Button
        type="button"
        variant="quiet"
        size="sm"
        className="h-7 rounded-full px-3 text-caption"
        disabled={disabled}
        onClick={() => void dispatchWorkspaceSurface("workspace")}
        data-testid="agent-action-open"
      >
        <FolderOpen className="size-3.5" />
        {labels.open}
      </Button>
      <Button
        type="button"
        variant="quiet"
        size="sm"
        className="h-7 rounded-full px-3 text-caption"
        disabled={disabled}
        onClick={() => void dispatchWorkspaceSurface("review")}
        data-testid="agent-action-review"
      >
        <FileSearch className="size-3.5" />
        {labels.review}
      </Button>
      <Button
        type="button"
        variant="pill"
        size="sm"
        className="h-7 rounded-full px-3 text-caption"
        onClick={() => onFeedback(`Continue from ${primaryArtifact.path}`)}
        data-testid="agent-action-feedback"
      >
        <MessageSquarePlus className="size-3.5" />
        {labels.feedback}
      </Button>
    </div>
  )
}
