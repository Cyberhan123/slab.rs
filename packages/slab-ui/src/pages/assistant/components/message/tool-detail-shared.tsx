"use client"

/**
 * Shared plumbing for the per-tool structured part renderers
 * (`message-tool-read-file-part` & siblings): envelope parsing, value
 * coercion, byte formatting, and the meta/footnote chrome every detail body
 * is laid out with.
 */

import type { ReactNode } from "react"

/** A tool result envelope, once unserialized. */
export type Envelope = Record<string, unknown>

/**
 * Normalize a tool output into a parsed object. Tool results arrive either as
 * the raw JSON string the tool produced or as the already-parsed value; plain
 * strings (older outputs, plain-text results) are NOT envelopes.
 */
export function parseToolEnvelope(output: unknown): Envelope | null {
  if (typeof output === "string") {
    if (!output.startsWith("{")) return null
    try {
      return parseToolEnvelope(JSON.parse(output))
    } catch {
      return null
    }
  }
  if (typeof output === "object" && output !== null && !Array.isArray(output)) {
    return output as Envelope
  }
  return null
}

/** Non-empty string or `undefined`. */
export const str = (value: unknown): string | undefined =>
  typeof value === "string" && value.length > 0 ? value : undefined

/** Finite number or `undefined`. */
export const num = (value: unknown): number | undefined =>
  typeof value === "number" && Number.isFinite(value) ? value : undefined

/** Human-readable byte size (B / KB / MB / GB), input-safe for huge values. */
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  const units = ["KB", "MB", "GB"]
  let value = bytes
  let unit = -1
  do {
    value /= 1024
    unit += 1
  } while (value >= 1024 && unit < units.length - 1)
  return `${value >= 10 ? Math.round(value) : value.toFixed(1)} ${units[unit]}`
}

/** Shared header/meta line above a detail body. */
export const DetailMeta = ({ children }: { children: ReactNode }) => (
  <p className="flex flex-wrap items-center gap-x-2 gap-y-0.5 font-mono text-muted-foreground text-xs">
    {children}
  </p>
)

/** Shared footnote for truncated envelopes. */
export const DetailFootnote = ({ children }: { children: ReactNode }) => (
  <p className="text-muted-foreground/80 text-xs italic">{children}</p>
)
