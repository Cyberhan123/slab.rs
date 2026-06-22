import type { ComponentProps } from "@ant-design/x-markdown"
import XMarkdown from "@ant-design/x-markdown"
import Latex from "@ant-design/x-markdown/plugins/latex"
import "@ant-design/x-markdown/dist/plugins/latex.css"
import { CodeHighlighter } from "@ant-design/x"
import { useClipboard } from "@mantine/hooks"
import { Copy } from "lucide-react"
import { isValidElement, memo, type ReactNode } from "react"
import { toast } from "sonner"

import { Button } from "@slab/components/button"
import { useTranslation } from "@slab/i18n"
import { cn } from "@/lib/utils"

type AssistantMarkdownProps = {
  children: string
  className?: string
  hasNextChunk?: boolean
}

function childrenToText(children: ReactNode): string {
  if (typeof children === "string" || typeof children === "number") {
    return String(children)
  }

  if (Array.isArray(children)) {
    return children.map(childrenToText).join("")
  }

  if (isValidElement<{ children?: ReactNode }>(children)) {
    return childrenToText(children.props.children)
  }

  return ""
}

function SupComponent({ children, ...props }: ComponentProps) {
  return (
    <sup
      {...props}
      className={cn(
        "mx-0.5 inline-flex min-w-4 items-center justify-center rounded-full border border-current/20 px-1 text-[0.7em] font-semibold leading-4 text-[color:var(--brand-teal)]",
        props.className
      )}
    >
      {children}
    </sup>
  )
}

function CodeBlockComponent({ block, children, lang, ...props }: ComponentProps) {
  const clipboard = useClipboard()
  const { t } = useTranslation()
  const code = childrenToText(children)
  if (!block) {
    return (
      <code {...props} className={cn("break-words whitespace-normal", props.className)}>
        {children}
      </code>
    )
  }

  const language = lang?.trim().split(/\s+/)[0] || "text"

  return (
    <CodeHighlighter
      lang={language}
      className="my-3 max-w-full overflow-x-auto rounded-[14px] border border-border/60 text-xs"
      header={
        <div className="flex min-w-0 items-center justify-between gap-3">
          <span className="min-w-0 truncate">{language}</span>
          <Button
            type="button"
            variant="quiet"
            size="icon-xs"
            className="size-6"
            data-testid="code-copy"
            aria-label="Copy code"
            onClick={() => {
              clipboard.copy(code)
              toast.success(t("pages.assistant.message.copied"))
            }}
          >
            <Copy className="size-3.5" />
          </Button>
        </div>
      }
    >
      {code}
    </CodeHighlighter>
  )
}

function ThinkComponent() {
  return null
}

const markdownComponents = {
  code: CodeBlockComponent,
  sup: SupComponent,
  think: ThinkComponent,
}

const markdownConfig = {
  extensions: Latex(),
}

const streamingActive = {
  enableAnimation: true,
  hasNextChunk: true,
}

function AssistantMarkdownView({
  children,
  className,
  hasNextChunk = false,
}: AssistantMarkdownProps) {
  return (
    <XMarkdown
      components={markdownComponents}
      config={markdownConfig}
      content={children}
      paragraphTag="div"
      className={cn(
        "assistant-markdown min-w-0 max-w-full overflow-hidden break-words text-base leading-[1.625]",
        className
      )}
      streaming={hasNextChunk ? streamingActive : undefined}
    />
  )
}

export const AssistantMarkdown = memo(AssistantMarkdownView)
