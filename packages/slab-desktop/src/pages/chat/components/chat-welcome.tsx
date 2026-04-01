import { ImagePlus, MessageSquarePlus, Sparkles } from "lucide-react"

import { Badge } from "@slab/components/badge"
import { Button } from "@slab/components/button"
import { Card, CardContent, CardHeader, CardTitle } from "@slab/components/card"

const SUGGESTED_PROMPTS = [
  "Summarize a research paper into key claims and open questions.",
  "Turn rough notes into a polished release announcement.",
  "Compare two local models for reasoning, speed, and memory usage.",
]

type ChatWelcomeProps = {
  agentName: string
  onUsePrompt: (prompt: string) => void
  onGenerateImage: () => void
  onNewSession: () => void
}

export function ChatWelcome({
  agentName,
  onUsePrompt,
  onGenerateImage,
  onNewSession,
}: ChatWelcomeProps) {
  return (
    <div className="flex flex-col gap-6">
      <Card variant="hero" className="workspace-halo overflow-hidden">
        <CardHeader className="gap-4">
          <div className="flex flex-wrap items-center gap-3">
            <Badge variant="chip">Chat workspace</Badge>
            <Sparkles className="size-4 text-[var(--brand-gold)]" />
          </div>
          <CardTitle className="max-w-2xl text-4xl leading-tight tracking-tight md:text-5xl">
            {agentName} is ready for drafting, reasoning, and fast prompt handoff.
          </CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-4 pt-0">
          <p className="max-w-2xl text-base leading-7 text-muted-foreground">
            Keep the thread focused here, then bounce promising prompts straight into image generation
            when the idea deserves a visual pass.
          </p>
          <div className="flex flex-wrap gap-3">
            <Button variant="cta" size="pill" onClick={onNewSession}>
              <MessageSquarePlus className="size-4" />
              New session
            </Button>
            <Button variant="pill" size="pill" onClick={onGenerateImage}>
              <ImagePlus className="size-4" />
              Go to image studio
            </Button>
          </div>
        </CardContent>
      </Card>

      <div className="grid gap-3">
        {SUGGESTED_PROMPTS.map((prompt) => (
          <button
            key={prompt}
            type="button"
            onClick={() => onUsePrompt(prompt)}
            className="workspace-soft-panel rounded-[24px] px-5 py-4 text-left transition hover:border-[color:var(--brand-teal)] hover:bg-[color:color-mix(in_oklab,var(--brand-teal)_6%,var(--surface-soft))]"
          >
            <p className="text-sm font-medium leading-6">{prompt}</p>
          </button>
        ))}
      </div>
    </div>
  )
}
