"use client"

/**
 * Local shiki-backed code block, re-implemented (trimmed) from the
 * `example-full-message/code-block.tsx` reference — that directory must not be
 * imported. Used to render tool-call parameters and results as syntax-highlighted
 * JSON with a copy button. The language-selector surface is intentionally
 * omitted (over-scope for tool I/O); markdown code fences are still handled by
 * Streamdown's own code plugin.
 */

import { Button } from "@slab/components/button"
import { cn } from "@/lib/utils"
import { CheckIcon, CopyIcon } from "lucide-react"
import type { ComponentProps, CSSProperties, HTMLAttributes } from "react"
import {
  createContext,
  memo,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react"
import type { BundledLanguage, BundledTheme, HighlighterGeneric, ThemedToken } from "shiki"
import { createHighlighter } from "shiki"

// Shiki font-style bitflags: 1=italic, 2=bold, 4=underline
// oxlint-disable-next-line eslint(no-bitwise)
const isItalic = (fontStyle: number | undefined) => fontStyle && fontStyle & 1
// oxlint-disable-next-line eslint(no-bitwise)
const isBold = (fontStyle: number | undefined) => fontStyle && fontStyle & 2
// oxlint-disable-next-line eslint(no-bitwise)
const isUnderline = (fontStyle: number | undefined) => fontStyle && fontStyle & 4

interface KeyedToken {
  token: ThemedToken
  key: string
}
interface KeyedLine {
  tokens: KeyedToken[]
  key: string
}

const addKeysToTokens = (lines: ThemedToken[][]): KeyedLine[] =>
  lines.map((line, lineIdx) => ({
    key: `line-${lineIdx}`,
    tokens: line.map((token, tokenIdx) => ({ key: `line-${lineIdx}-${tokenIdx}`, token })),
  }))

const TokenSpan = ({ token }: { token: ThemedToken }) => (
  <span
    className="dark:!bg-[var(--shiki-dark-bg)] dark:!text-[var(--shiki-dark)]"
    style={
      {
        backgroundColor: token.bgColor,
        color: token.color,
        fontStyle: isItalic(token.fontStyle) ? "italic" : undefined,
        fontWeight: isBold(token.fontStyle) ? "bold" : undefined,
        textDecoration: isUnderline(token.fontStyle) ? "underline" : undefined,
        ...token.htmlStyle,
      } as CSSProperties
    }
  >
    {token.content}
  </span>
)

const LINE_NUMBER_CLASSES = cn(
  "block",
  "before:content-[counter(line)]",
  "before:inline-block",
  "before:[counter-increment:line]",
  "before:w-8",
  "before:mr-4",
  "before:text-right",
  "before:text-muted-foreground/50",
  "before:font-mono",
  "before:select-none",
)

const LineSpan = ({
  keyedLine,
  showLineNumbers,
}: {
  keyedLine: KeyedLine
  showLineNumbers: boolean
}) => (
  <span className={showLineNumbers ? LINE_NUMBER_CLASSES : "block"}>
    {keyedLine.tokens.length === 0
      ? "\n"
      : keyedLine.tokens.map(({ token, key }) => <TokenSpan key={key} token={token} />)}
  </span>
)

type CodeBlockProps = HTMLAttributes<HTMLDivElement> & {
  code: string
  language: BundledLanguage
  showLineNumbers?: boolean
}

interface TokenizedCode {
  tokens: ThemedToken[][]
  fg: string
  bg: string
}

const CodeBlockContext = createContext<{ code: string }>({ code: "" })

const highlighterCache = new Map<string, Promise<HighlighterGeneric<BundledLanguage, BundledTheme>>>()
const tokensCache = new Map<string, TokenizedCode>()
const subscribers = new Map<string, Set<(result: TokenizedCode) => void>>()

const getTokensCacheKey = (code: string, language: BundledLanguage) => {
  const start = code.slice(0, 100)
  const end = code.length > 100 ? code.slice(-100) : ""
  return `${language}:${code.length}:${start}:${end}`
}

const getHighlighter = (
  language: BundledLanguage,
): Promise<HighlighterGeneric<BundledLanguage, BundledTheme>> => {
  const cached = highlighterCache.get(language)
  if (cached) return cached
  const promise = createHighlighter({ langs: [language], themes: ["github-light", "github-dark"] })
  highlighterCache.set(language, promise)
  return promise
}

const createRawTokens = (code: string): TokenizedCode => ({
  bg: "transparent",
  fg: "inherit",
  tokens: code.split("\n").map((line) =>
    line === "" ? [] : [{ color: "inherit", content: line } as ThemedToken],
  ),
})

/** Synchronous highlight with a callback for async (shiki-load) results. */
export const highlightCode = (
  code: string,
  language: BundledLanguage,
  // oxlint-disable-next-line eslint-plugin/promise(prefer-await-to-callbacks)
  callback?: (result: TokenizedCode) => void,
): TokenizedCode | null => {
  const cacheKey = getTokensCacheKey(code, language)
  const cached = tokensCache.get(cacheKey)
  if (cached) return cached

  if (callback) {
    if (!subscribers.has(cacheKey)) subscribers.set(cacheKey, new Set())
    subscribers.get(cacheKey)?.add(callback)
  }

  getHighlighter(language)
    // oxlint-disable-next-line eslint-plugin/promise(prefer-await-to-then)
    .then((highlighter) => {
      const available = highlighter.getLoadedLanguages()
      const langToUse = available.includes(language) ? language : "text"
      const result = highlighter.codeToTokens(code, {
        lang: langToUse,
        themes: { dark: "github-dark", light: "github-light" },
      })
      const tokenized: TokenizedCode = {
        bg: result.bg ?? "transparent",
        fg: result.fg ?? "inherit",
        tokens: result.tokens,
      }
      tokensCache.set(cacheKey, tokenized)
      const subs = subscribers.get(cacheKey)
      if (subs) {
        for (const sub of subs) sub(tokenized)
        subscribers.delete(cacheKey)
      }
    })
    // oxlint-disable-next-line eslint-plugin/promise(prefer-await-to-then), eslint-plugin/promise(prefer-await-to-callbacks)
    .catch((error) => {
      console.error("Failed to highlight code:", error)
      subscribers.delete(cacheKey)
    })

  return null
}

const CodeBlockBody = memo(
  ({
    tokenized,
    showLineNumbers,
    className,
  }: {
    tokenized: TokenizedCode
    showLineNumbers: boolean
    className?: string
  }) => {
    const preStyle = useMemo(
      () => ({ backgroundColor: tokenized.bg, color: tokenized.fg }),
      [tokenized.bg, tokenized.fg],
    )
    const keyedLines = useMemo(() => addKeysToTokens(tokenized.tokens), [tokenized.tokens])
    return (
      <pre
        className={cn("dark:!bg-[var(--shiki-dark-bg)] dark:!text-[var(--shiki-dark)] m-0 p-4 text-sm", className)}
        style={preStyle}
      >
        <code
          className={cn(
            "font-mono text-sm",
            showLineNumbers && "[counter-increment:line_0] [counter-reset:line]",
          )}
        >
          {keyedLines.map((keyedLine) => (
            <LineSpan key={keyedLine.key} keyedLine={keyedLine} showLineNumbers={showLineNumbers} />
          ))}
        </code>
      </pre>
    )
  },
  (prev, next) =>
    prev.tokenized === next.tokenized &&
    prev.showLineNumbers === next.showLineNumbers &&
    prev.className === next.className,
)

CodeBlockBody.displayName = "CodeBlockBody"

const CodeBlockContent = ({
  code,
  language,
  showLineNumbers = false,
}: {
  code: string
  language: BundledLanguage
  showLineNumbers?: boolean
}) => {
  const rawTokens = useMemo(() => createRawTokens(code), [code])
  const syncTokens = useMemo(() => highlightCode(code, language) ?? rawTokens, [code, language, rawTokens])
  const [asyncTokens, setAsyncTokens] = useState<TokenizedCode | null>(null)
  const asyncKeyRef = useRef({ code, language })

  if (asyncKeyRef.current.code !== code || asyncKeyRef.current.language !== language) {
    asyncKeyRef.current = { code, language }
    setAsyncTokens(null)
  }

  useEffect(() => {
    let cancelled = false
    highlightCode(code, language, (result) => {
      if (!cancelled) setAsyncTokens(result)
    })
    return () => {
      cancelled = true
    }
  }, [code, language])

  return (
    <div className="relative overflow-auto">
      <CodeBlockBody showLineNumbers={showLineNumbers} tokenized={asyncTokens ?? syncTokens} />
    </div>
  )
}

export const CodeBlock = ({
  code,
  language,
  showLineNumbers = false,
  className,
  children,
  ...props
}: CodeBlockProps) => {
  const contextValue = useMemo(() => ({ code }), [code])
  return (
    <CodeBlockContext.Provider value={contextValue}>
      <div
        className={cn(
          "group relative w-full overflow-hidden rounded-md border bg-background text-foreground",
          className,
        )}
        data-language={language}
        style={{ containIntrinsicSize: "auto 200px", contentVisibility: "auto" }}
        {...props}
      >
        {children}
        <CodeBlockContent code={code} language={language} showLineNumbers={showLineNumbers} />
      </div>
    </CodeBlockContext.Provider>
  )
}

export type CodeBlockCopyButtonProps = ComponentProps<typeof Button> & {
  onCopy?: () => void
  onError?: (error: Error) => void
  timeout?: number
}

export const CodeBlockCopyButton = ({
  onCopy,
  onError,
  timeout = 2000,
  children,
  className,
  ...props
}: CodeBlockCopyButtonProps) => {
  const [isCopied, setIsCopied] = useState(false)
  const timeoutRef = useRef<number>(0)
  const { code } = useContext(CodeBlockContext)

  const copyToClipboard = async () => {
    if (typeof window === "undefined" || !navigator?.clipboard?.writeText) {
      onError?.(new Error("Clipboard API not available"))
      return
    }
    try {
      if (!isCopied) {
        await navigator.clipboard.writeText(code)
        setIsCopied(true)
        onCopy?.()
        timeoutRef.current = window.setTimeout(() => setIsCopied(false), timeout)
      }
    } catch (error) {
      onError?.(error as Error)
    }
  }

  useEffect(
    () => () => {
      window.clearTimeout(timeoutRef.current)
    },
    [],
  )

  const Icon = isCopied ? CheckIcon : CopyIcon
  return (
    <Button className={cn("shrink-0", className)} onClick={copyToClipboard} size="icon" variant="ghost" {...props}>
      {children ?? <Icon size={14} />}
    </Button>
  )
}

