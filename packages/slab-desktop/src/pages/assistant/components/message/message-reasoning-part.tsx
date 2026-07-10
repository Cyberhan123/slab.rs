"use client"

import { useControllableState } from "@radix-ui/react-use-controllable-state"
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@slab/components/collapsible"
import { cn } from "@/lib/utils"
import { BrainIcon, ChevronDownIcon } from "lucide-react"
import type { ComponentProps, ReactNode } from "react"
import {
  createContext,
  memo,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react"

import { Markdown } from "./markdown"
import { Shimmer } from "./shimmer"
import type { MessagePartRenderProps } from "./message-parts"
import type { TMessage, TMessagePart } from "./message-item"

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

const AUTO_CLOSE_DELAY = 1000
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
    const resolvedDefaultOpen = defaultOpen ?? isStreaming
    // Track if defaultOpen was explicitly set to false (to prevent auto-open).
    const isExplicitlyClosed = defaultOpen === false

    const [isOpen, setIsOpen] = useControllableState<boolean>({
      defaultProp: resolvedDefaultOpen,
      onChange: onOpenChange,
      prop: open,
    })
    const [duration, setDuration] = useControllableState<number | undefined>({
      defaultProp: undefined,
      prop: durationProp,
    })

    const hasEverStreamedRef = useRef(isStreaming)
    const [hasAutoClosed, setHasAutoClosed] = useState(false)
    const startTimeRef = useRef<number | null>(null)

    // Track when streaming starts and compute duration once it ends.
    useEffect(() => {
      if (isStreaming) {
        hasEverStreamedRef.current = true
        if (startTimeRef.current === null) startTimeRef.current = Date.now()
      } else if (startTimeRef.current !== null) {
        setDuration(Math.ceil((Date.now() - startTimeRef.current) / MS_IN_S))
        startTimeRef.current = null
      }
    }, [isStreaming, setDuration])

    // Auto-open when streaming starts (unless explicitly closed).
    useEffect(() => {
      if (isStreaming && !isOpen && !isExplicitlyClosed) setIsOpen(true)
    }, [isStreaming, isOpen, setIsOpen, isExplicitlyClosed])

    // Auto-close when streaming ends (once only, and only if it ever streamed).
    useEffect(() => {
      if (hasEverStreamedRef.current && !isStreaming && isOpen && !hasAutoClosed) {
        const timer = setTimeout(() => {
          setIsOpen(false)
          setHasAutoClosed(true)
        }, AUTO_CLOSE_DELAY)
        return () => clearTimeout(timer)
      }
    }, [isStreaming, isOpen, setIsOpen, hasAutoClosed])

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

const defaultGetThinkingMessage = (isStreaming: boolean, duration?: number): ReactNode => {
  if (isStreaming || duration === 0) return <Shimmer duration={1}>Thinking...</Shimmer>
  if (duration === undefined) return <p>Thought for a few seconds</p>
  return <p>Thought for {duration} seconds</p>
}

type ReasoningTriggerProps = ComponentProps<typeof CollapsibleTrigger> & {
  getThinkingMessage?: (isStreaming: boolean, duration?: number) => ReactNode
}

const ReasoningTrigger = memo(
  ({ className, children, getThinkingMessage = defaultGetThinkingMessage, ...props }: ReasoningTriggerProps) => {
    const { isStreaming, isOpen, duration } = useReasoning()
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
            {getThinkingMessage(isStreaming, duration)}
            <ChevronDownIcon
              className={cn("size-4 transition-transform", isOpen ? "rotate-180" : "rotate-0")}
            />
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
