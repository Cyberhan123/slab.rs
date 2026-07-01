import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { ChevronDown, ChevronUp, ExternalLink, Film, Mic, Pin, PinOff, X } from "lucide-react"
import { useNavigate } from "react-router-dom"

import {
  A2uHubSurface,
  A2uImageSurface,
  A2uPluginSurface,
  A2uReviewSurface,
  A2uSurfaceFrame,
  A2uWorkspaceSurface,
  type A2uSurfaceAction,
} from "@slab/components/a2u"
import { Button } from "@slab/components/button"
import { useTranslation } from "@slab/i18n"
import { openSurfaceWindow, type SurfaceKind } from "@/lib/windows"
import { useAgentSurfaceStore, type AgentSurfaceRequest } from "@/store/useAgentSurfaceStore"

function imageRouteSearch(prompt?: string) {
  return prompt ? `?prompt=${encodeURIComponent(prompt)}` : ""
}

const WINDOW_SURFACE_KINDS: readonly SurfaceKind[] = [
  "workspace",
  "image",
  "review",
  "plugin",
  "hub",
]

/** Narrows a surface type to a kind that can open in its own OS window. */
function isWindowSurfaceKind(type: string): type is SurfaceKind {
  return (WINDOW_SURFACE_KINDS as readonly string[]).includes(type)
}

type AgentSurfaceLayerProps = {
  onActiveChange?: (active: boolean) => void
  onSurfaceClosed?: () => void
  variant?: "inline" | "shell"
}

export function AgentSurfaceLayer({
  onActiveChange,
  onSurfaceClosed,
  variant = "inline",
}: AgentSurfaceLayerProps) {
  const navigate = useNavigate()
  const { t } = useTranslation()
  const pendingSurface = useAgentSurfaceStore((state) => state.pendingSurface)
  const consumePendingSurface = useAgentSurfaceStore((state) => state.consumePendingSurface)
  const setPendingSurface = useAgentSurfaceStore((state) => state.setPendingSurface)
  const [activeSurface, setActiveSurface] = useState<AgentSurfaceRequest | null>(null)
  const [surfaceCollapsed, setSurfaceCollapsed] = useState(false)
  const [surfacePinned, setSurfacePinned] = useState(false)
  const [surfaceAnnouncement, setSurfaceAnnouncement] = useState("")
  const surfaceRef = useRef<HTMLDivElement | null>(null)

  useEffect(() => {
    if (!pendingSurface) {
      return
    }

    // Window-targeted surfaces are dispatched to their own OS window (INFRA-11),
    // not rendered inline.
    if (pendingSurface.target === "window") {
      const request = consumePendingSurface(pendingSurface.id)
      if (request && isWindowSurfaceKind(request.type)) {
        void openSurfaceWindow(request.type, request.id)
      }
      return
    }

    if (pendingSurface.targetRoute && pendingSurface.targetRoute !== "assistant") {
      return
    }

    if (surfacePinned && activeSurface) {
      return
    }

    const request = consumePendingSurface(pendingSurface.id)
    if (request) {
      setActiveSurface(request)
      setSurfaceCollapsed(false)
    }
  }, [activeSurface, consumePendingSurface, pendingSurface, surfacePinned])

  useEffect(() => {
    if (activeSurface) {
      setSurfaceAnnouncement(t("pages.assistant.surface.opened"))
      surfaceRef.current?.focus()
    }
  }, [activeSurface, t])

  useEffect(() => {
    onActiveChange?.(Boolean(activeSurface))
  }, [activeSurface, onActiveChange])

  const closeSurface = useCallback(() => {
    setActiveSurface(null)
    setSurfaceCollapsed(false)
    setSurfacePinned(false)
    setSurfaceAnnouncement(t("pages.assistant.surface.closed"))
    onSurfaceClosed?.()
  }, [onSurfaceClosed, t])

  const toggleSurfaceCollapsed = useCallback(() => {
    setSurfaceCollapsed((value) => !value)
  }, [])

  const toggleSurfacePinned = useCallback(() => {
    setSurfacePinned((value) => !value)
  }, [])

  useEffect(() => {
    if (!activeSurface) {
      return undefined
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        closeSurface()
      }
    }

    window.addEventListener("keydown", handleKeyDown)
    return () => window.removeEventListener("keydown", handleKeyDown)
  }, [activeSurface, closeSurface])

  const openWorkspace = useCallback(() => {
    if (!activeSurface || activeSurface.type !== "workspace") {
      return
    }

    setPendingSurface({
      type: "workspace",
      payload: activeSurface.payload,
    }, {
      targetRoute: "workspace",
    })
    closeSurface()
    navigate("/workspace")
  }, [activeSurface, closeSurface, navigate, setPendingSurface])

  const openImage = useCallback(() => {
    if (!activeSurface || activeSurface.type !== "image") {
      return
    }

    closeSurface()
    navigate({
      pathname: "/image",
      search: imageRouteSearch(activeSurface.payload?.prompt),
    })
  }, [activeSurface, closeSurface, navigate])

  const openHub = useCallback(() => {
    closeSurface()
    navigate("/hub")
  }, [closeSurface, navigate])

  const openAudio = useCallback(() => {
    closeSurface()
    navigate("/audio")
  }, [closeSurface, navigate])

  const openPlugins = useCallback(() => {
    closeSurface()
    navigate("/plugins")
  }, [closeSurface, navigate])

  const openVideo = useCallback(() => {
    closeSurface()
    navigate("/video")
  }, [closeSurface, navigate])

  const actions = useMemo<A2uSurfaceAction[]>(() => {
    if (!activeSurface) {
      return []
    }

    switch (activeSurface.type) {
      case "workspace":
        return [
          {
            icon: ExternalLink,
            label: t("pages.assistant.surface.openInWorkspace"),
            onClick: openWorkspace,
            testId: "agent-surface-open-workspace",
          },
        ]
      case "image":
        return [
          {
            icon: ExternalLink,
            label: t("pages.assistant.surface.openInImage"),
            onClick: openImage,
            testId: "agent-surface-open-image",
          },
        ]
      case "hub":
        return [
          {
            icon: ExternalLink,
            label: t("pages.assistant.surface.openHub"),
            onClick: openHub,
            testId: "agent-surface-open-hub",
          },
        ]
      case "audio":
        return [
          {
            icon: ExternalLink,
            label: t("pages.assistant.surface.openAudio"),
            onClick: openAudio,
            testId: "agent-surface-open-audio",
          },
        ]
      case "plugin":
        return [
          {
            icon: ExternalLink,
            label: t("pages.assistant.surface.openPlugins"),
            onClick: openPlugins,
            testId: "agent-surface-open-plugins",
          },
        ]
      case "video":
        return [
          {
            icon: ExternalLink,
            label: t("pages.assistant.surface.openVideo"),
            onClick: openVideo,
            testId: "agent-surface-open-video",
          },
        ]
      default:
        return []
    }
  }, [activeSurface, openAudio, openHub, openImage, openPlugins, openVideo, openWorkspace, t])

  const liveRegion = (
    <div
      className="sr-only"
      aria-live="polite"
      aria-atomic="true"
      data-testid="agent-surface-live-region"
    >
      {surfaceAnnouncement}
    </div>
  )

  if (!activeSurface) {
    return liveRegion
  }

  return (
    <>
      {liveRegion}
      <section
        ref={surfaceRef}
        tabIndex={-1}
        aria-label={t("pages.assistant.surface.regionLabel")}
        className={variant === "shell"
          ? "flex min-h-0 flex-1 px-[var(--shell-content-gutter)] pb-[var(--shell-content-gutter)] pt-4"
          : "mx-auto w-full max-w-[768px] px-6 pb-4 md:px-8 lg:px-0"}
        data-testid="agent-surface-layer"
      >
        <div
          className={variant === "shell"
            ? "relative flex min-h-0 w-full flex-1 flex-col overflow-hidden"
            : "relative"}
          data-testid="agent-surface-frame"
        >
          <div className="absolute right-3 top-3 z-10 flex items-center gap-1">
            <Button
              type="button"
              variant="quiet"
              size="icon-sm"
              aria-label={surfacePinned
                ? t("pages.assistant.surface.unpin")
                : t("pages.assistant.surface.pin")}
              aria-pressed={surfacePinned}
              className="bg-[var(--surface-1)]"
              onClick={toggleSurfacePinned}
              data-testid="agent-surface-pin"
            >
              {surfacePinned ? <PinOff className="size-4" /> : <Pin className="size-4" />}
            </Button>
            <Button
              type="button"
              variant="quiet"
              size="icon-sm"
              aria-label={surfaceCollapsed
                ? t("pages.assistant.surface.expand")
                : t("pages.assistant.surface.collapse")}
              aria-expanded={!surfaceCollapsed}
              aria-controls="agent-surface-content"
              className="bg-[var(--surface-1)]"
              onClick={toggleSurfaceCollapsed}
              data-testid="agent-surface-collapse"
            >
              {surfaceCollapsed ? <ChevronUp className="size-4" /> : <ChevronDown className="size-4" />}
            </Button>
            <Button
              type="button"
              variant="quiet"
              size="icon-sm"
              aria-label={t("pages.assistant.surface.close")}
              className="bg-[var(--surface-1)]"
              onClick={closeSurface}
              data-testid="agent-surface-close"
            >
              <X className="size-4" />
            </Button>
          </div>
          {surfacePinned ? (
            <div
              className="absolute left-3 top-3 z-10 rounded-full border border-border/70 bg-[var(--surface-1)] px-2.5 py-1 text-micro font-semibold text-muted-foreground"
              data-testid="agent-surface-pinned-indicator"
            >
              {t("pages.assistant.surface.pinned")}
            </div>
          ) : null}
          {surfaceCollapsed ? (
            <div
              id="agent-surface-content"
              className="rounded-[20px] border border-border/60 bg-[var(--surface-soft)] px-5 py-4 text-sm font-medium text-muted-foreground"
              data-testid="agent-surface-collapsed"
            >
              {t("pages.assistant.surface.collapsed")}
            </div>
          ) : (
            <div
              id="agent-surface-content"
              className={variant === "shell" ? "min-h-0 flex-1 overflow-auto" : undefined}
              data-testid="agent-surface-content"
            >
              {activeSurface.type === "workspace" ? (
                <A2uWorkspaceSurface
                  actions={actions}
                  revealPath={activeSurface.payload?.revealPath}
                  labels={{
                    description: t("pages.assistant.surface.workspace.description"),
                    emptyDescription: t("pages.assistant.surface.workspace.emptyDescription"),
                    revealPath: t("pages.assistant.surface.workspace.revealPath"),
                    title: t("pages.assistant.surface.workspace.title"),
                  }}
                />
              ) : null}
              {activeSurface.type === "image" ? (
                <A2uImageSurface
                  actions={actions}
                  prompt={activeSurface.payload?.prompt}
                  labels={{
                    description: t("pages.assistant.surface.image.description"),
                    emptyDescription: t("pages.assistant.surface.image.emptyDescription"),
                    prompt: t("pages.assistant.surface.image.prompt"),
                    title: t("pages.assistant.surface.image.title"),
                  }}
                />
              ) : null}
              {activeSurface.type === "review" ? (
                <A2uReviewSurface
                  actions={actions}
                  diff={activeSurface.payload?.diff}
                  path={activeSurface.payload?.path}
                  labels={{
                    description: t("pages.assistant.surface.review.description"),
                    diff: t("pages.assistant.surface.review.diff"),
                    emptyDescription: t("pages.assistant.surface.review.emptyDescription"),
                    path: t("pages.assistant.surface.review.path"),
                    title: t("pages.assistant.surface.review.title"),
                  }}
                />
              ) : null}
              {activeSurface.type === "plugin" ? (
                <A2uPluginSurface
                  actions={actions}
                  pluginId={activeSurface.payload?.pluginId}
                  surface={activeSurface.payload?.surface}
                  labels={{
                    description: t("pages.assistant.surface.plugin.description"),
                    emptyDescription: t("pages.assistant.surface.plugin.emptyDescription"),
                    pluginId: t("pages.assistant.surface.plugin.pluginId"),
                    surface: t("pages.assistant.surface.plugin.surface"),
                    title: t("pages.assistant.surface.plugin.title"),
                  }}
                />
              ) : null}
              {activeSurface.type === "hub" ? (
                <A2uHubSurface
                  actions={actions}
                  labels={{
                    description: t("pages.assistant.surface.hub.description"),
                    title: t("pages.assistant.surface.hub.title"),
                  }}
                />
              ) : null}
              {activeSurface.type === "audio" ? (
                <A2uSurfaceFrame
                  actions={actions}
                  data-testid="a2u-audio-surface"
                  icon={Mic}
                  title={t("pages.assistant.surface.audio.title")}
                  description={t("pages.assistant.surface.audio.description")}
                />
              ) : null}
              {activeSurface.type === "video" ? (
                <A2uSurfaceFrame
                  actions={actions}
                  data-testid="a2u-video-surface"
                  icon={Film}
                  title={t("pages.assistant.surface.video.title")}
                  description={t("pages.assistant.surface.video.description")}
                />
              ) : null}
            </div>
          )}
        </div>
      </section>
    </>
  )
}
