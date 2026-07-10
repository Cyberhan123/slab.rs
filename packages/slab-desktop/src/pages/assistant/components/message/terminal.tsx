"use client"

/**
 * Terminal surface for command-execution output, re-implemented (trimmed) from
 * the `example-full-message/terminal.tsx` reference — that directory must not be
 * imported. Renders ANSI-colored output with auto-scroll, a streaming cursor,
 * and a copy button (Mantine `useClipboard`, no hand-rolled clipboard).
 */

import { Button } from "@slab/components/button"
import { useClipboard } from "@mantine/hooks"
import { cn } from "@/lib/utils"
import Ansi from "./ansi-to-react"
import { CheckIcon, CopyIcon, TerminalIcon, Trash2Icon } from "lucide-react"
import type { ComponentProps, HTMLAttributes } from "react"
import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useRef,
} from "react"

interface TerminalContextValue {
  output: string
  isStreaming: boolean
  autoScroll: boolean
  onClear?: () => void
}

const TerminalContext = createContext<TerminalContextValue>({
  autoScroll: true,
  isStreaming: false,
  output: "",
})

export type TerminalHeaderProps = HTMLAttributes<HTMLDivElement>

export const TerminalHeader = ({ className, children, ...props }: TerminalHeaderProps) => (
  <div
    className={cn("flex items-center justify-between border-b border-zinc-800 px-4 py-2", className)}
    {...props}
  >
    {children}
  </div>
)

export type TerminalTitleProps = HTMLAttributes<HTMLDivElement>

export const TerminalTitle = ({ className, children, ...props }: TerminalTitleProps) => (
  <div className={cn("flex items-center gap-2 text-sm text-zinc-400", className)} {...props}>
    <TerminalIcon className="size-4" />
    {children ?? "Terminal"}
  </div>
)

export type TerminalStatusProps = HTMLAttributes<HTMLDivElement>

export const TerminalStatus = ({ className, children, ...props }: TerminalStatusProps) => {
  const { isStreaming } = useContext(TerminalContext)
  if (!isStreaming) return null
  return (
    <div className={cn("flex items-center gap-2 text-xs text-zinc-400", className)} {...props}>
      {children}
    </div>
  )
}

export type TerminalActionsProps = HTMLAttributes<HTMLDivElement>

export const TerminalActions = ({ className, children, ...props }: TerminalActionsProps) => (
  <div className={cn("flex items-center gap-1", className)} {...props}>
    {children}
  </div>
)

export type TerminalCopyButtonProps = ComponentProps<typeof Button> & {
  timeout?: number
}

export const TerminalCopyButton = ({
  timeout = 2000,
  children,
  className,
  ...props
}: TerminalCopyButtonProps) => {
  const { output } = useContext(TerminalContext)
  const clipboard = useClipboard({ timeout })

  return (
    <Button
      className={cn("size-7 shrink-0 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100", className)}
      onClick={() => clipboard.copy(output)}
      size="icon"
      variant="ghost"
      {...props}
    >
      {children ?? (clipboard.copied ? <CheckIcon size={14} /> : <CopyIcon size={14} />)}
    </Button>
  )
}

export type TerminalClearButtonProps = ComponentProps<typeof Button>

export const TerminalClearButton = ({ children, className, ...props }: TerminalClearButtonProps) => {
  const { onClear } = useContext(TerminalContext)
  if (!onClear) return null
  return (
    <Button
      className={cn("size-7 shrink-0 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100", className)}
      onClick={onClear}
      size="icon"
      variant="ghost"
      {...props}
    >
      {children ?? <Trash2Icon size={14} />}
    </Button>
  )
}

export type TerminalContentProps = HTMLAttributes<HTMLDivElement>

export const TerminalContent = ({ className, children, ...props }: TerminalContentProps) => {
  const { output, isStreaming, autoScroll } = useContext(TerminalContext)
  const containerRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (autoScroll && containerRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight
    }
  }, [output, autoScroll])
  
  return (
    <div
      className={cn("max-h-96 overflow-auto p-4 font-mono text-sm leading-relaxed", className)}
      ref={containerRef}
      {...props}
    >
      {children ?? (
        <pre className="break-words whitespace-pre-wrap">
          <Ansi>{output}</Ansi>
          {isStreaming ? <span className="ml-0.5 inline-block h-4 w-2 animate-pulse bg-zinc-100" /> : null}
        </pre>
      )}
    </div>
  )
}

export type TerminalProps = HTMLAttributes<HTMLDivElement> & {
  output: string
  isStreaming?: boolean
  autoScroll?: boolean
  onClear?: () => void
}

export const Terminal = ({
  output,
  isStreaming = false,
  autoScroll = true,
  onClear,
  className,
  children,
  ...props
}: TerminalProps) => {
  const contextValue = useMemo(
    () => ({ autoScroll, isStreaming, onClear, output }),
    [autoScroll, isStreaming, onClear, output],
  )

  return (
    <TerminalContext.Provider value={contextValue}>
      <div
        className={cn(
          "flex flex-col overflow-hidden rounded-lg border bg-zinc-950 text-zinc-100",
          className,
        )}
        {...props}
      >
        {children ?? (
          <>
            <TerminalHeader>
              <TerminalTitle />
              <div className="flex items-center gap-1">
                <TerminalStatus />
                <TerminalActions>
                  <TerminalCopyButton />
                  {onClear ? <TerminalClearButton /> : null}
                </TerminalActions>
              </div>
            </TerminalHeader>
            <TerminalContent />
          </>
        )}
      </div>
    </TerminalContext.Provider>
  )
}
