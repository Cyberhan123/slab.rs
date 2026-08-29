"use client"

import { useControllableState } from "@radix-ui/react-use-controllable-state"
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@slab/components/collapsible"
import { cn } from "@slab/ui/lib/utils"
import { BrainIcon, ChevronDownIcon, ChevronRightIcon } from "lucide-react"
import type { ComponentProps, ReactNode } from "react"
import {
  createContext,
  memo,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
} from "react"

import { Markdown } from "./markdown"
import { Shimmer } from "./shimmer"
import type { MessagePartRenderProps } from "./message-parts"
import type { TMessage, TMessagePart } from "./message-item"
import { useTranslation } from "@slab/i18n"

interface ReasoningContextValue {
  isStreaming: boolean
  isOpen: boolean
  setIsOpen: (open: boolean) => void
  duration: number | undefined
}

const ReasoningContext = createContext<ReasoningContextValue | null>(null)

const useReasoning = () => {
  const context = useContext(ReasoningContext)
  if (!context) {
    throw new Error("Reasoning components must be used within Reasoning")
  }
  return context
}

type ReasoningProps = ComponentProps<typeof Collapsible> & {
  isStreaming?: boolean
  open?: boolean
  defaultOpen?: boolean
  onOpenChange?: (open: boolean) => void
  duration?: number
}

const MS_IN_S = 1000

const Reasoning = memo(
  ({
    className,
    isStreaming = false,
    open,
    defaultOpen,
    onOpenChange,
    duration: durationProp,
    children,
    ...props
  }: ReasoningProps) => {
    // Collapsed by default — during generation and after — so the transcript
    // stays compact; the user expands via the trigger whenever they want.
    const resolvedDefaultOpen = defaultOpen ?? false

    const [isOpen, setIsOpen] = useControllableState<boolean>({
      defaultProp: resolvedDefaultOpen,
      onChange: onOpenChange,
      prop: open,
    })
    const [duration, setDuration] = useControllableState<number | undefined>({
      defaultProp: undefined,
      prop: durationProp,
    })

    const startTimeRef = useRef<number | null>(null)

    // Track when streaming starts and compute duration once it ends.
    useEffect(() => {
      if (isStreaming) {
        if (startTimeRef.current === null) startTimeRef.current = Date.now()
      } else if (startTimeRef.current !== null) {
        setDuration(Math.ceil((Date.now() - startTimeRef.current) / MS_IN_S))
        startTimeRef.current = null
      }
    }, [isStreaming, setDuration])

    const handleOpenChange = useCallback(
      (nextOpen: boolean) => {
        setIsOpen(nextOpen)
      },
      [setIsOpen],
    )

    const contextValue = useMemo(
      () => ({ duration, isOpen, isStreaming, setIsOpen }),
      [duration, isOpen, isStreaming, setIsOpen],
    )

    return (
      <ReasoningContext.Provider value={contextValue}>
        <Collapsible
          className={cn("not-prose mb-2", className)}
          onOpenChange={handleOpenChange}
          open={isOpen}
          {...props}
        >
          {children}
        </Collapsible>
      </ReasoningContext.Provider>
    )
  },
)
Reasoning.displayName = "Reasoning"

type ReasoningTriggerProps = ComponentProps<typeof CollapsibleTrigger> & {
  getThinkingMessage?: (isStreaming: boolean, duration?: number) => ReactNode
}

const ReasoningTrigger = memo(
  ({ className, children, getThinkingMessage, ...props }: ReasoningTriggerProps) => {
    const { isStreaming, isOpen, duration } = useReasoning()
    const { t } = useTranslation()
    const message =
      getThinkingMessage?.(isStreaming, duration) ??
      (isStreaming || duration === 0 ? (
        <Shimmer duration={1}>{t("pages.assistant.thinking.loading")}</Shimmer>
      ) : duration === undefined ? (
        <p>{t("pages.assistant.thinking.thoughtForAFewSeconds")}</p>
      ) : (
        <p>{t("pages.assistant.thinking.thoughtForSeconds", { seconds: duration })}</p>
      ))
    return (
      <CollapsibleTrigger
        className={cn(
          "flex w-full items-center gap-2 text-muted-foreground text-sm transition-colors hover:text-foreground",
          className,
        )}
        {...props}
      >
        {children ?? (
          <>
            <BrainIcon className="size-4" />
            {message}
            {isOpen ? (
              <ChevronDownIcon className="size-4 shrink-0" />
            ) : (
              <ChevronRightIcon className="size-4 shrink-0" />
            )}
          </>
        )}
      </CollapsibleTrigger>
    )
  },
)
ReasoningTrigger.displayName = "ReasoningTrigger"

type ReasoningContentProps = ComponentProps<typeof CollapsibleContent>

const ReasoningContent = memo(({ className, children, ...props }: ReasoningContentProps) => {
  const { isStreaming } = useReasoning()
  return (
    <CollapsibleContent
      className={cn(
        "mt-3 text-sm",
        "data-[state=closed]:fade-out-0 data-[state=closed]:slide-out-to-top-2 data-[state=open]:slide-in-from-top-2 text-muted-foreground outline-none data-[state=closed]:animate-out data-[state=open]:animate-in",
        className,
      )}
      {...props}
    >
      <Markdown hasNextChunk={isStreaming}>{children as string}</Markdown>
    </CollapsibleContent>
  )
})
ReasoningContent.displayName = "ReasoningContent"

function MessageReasoningPart({
  part,
  kind,
}: MessagePartRenderProps<TMessagePart, TMessage>) {
  if (kind !== "reasoning") return null

  const text = (part.text as string | undefined) ?? ""
  const isStreaming = part.state === "streaming"

  return (
    <Reasoning isStreaming={isStreaming}>
      <ReasoningTrigger />
      {text ? <ReasoningContent>{text}</ReasoningContent> : null}
    </Reasoning>
  )
}

export default MessageReasoningPart
