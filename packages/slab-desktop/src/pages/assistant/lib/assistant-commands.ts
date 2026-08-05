const COMMAND_PREFIX = "/"

/** True when the composer input is the `/compact` control command (never reaches the model). */
export function isCompactCommand(value: string): boolean {
    return value.trim() === "/compact"
}

/** True when the composer input is the `/fork` control command (never reaches the model). */
export function isForkCommand(value: string): boolean {
    return value.trim() === "/fork"
}

export type AssistantCommandParseResult = { name: string; args: string } | null

/**
 * Parse a leading `/<name>` control command. Returns `null` when the input is
 * not a command (no leading slash, or a bare slash). The first token after the
 * slash is the command name; the remaining tokens (joined by a single space)
 * are its args.
 */
export function parseAssistantCommand(value: string): AssistantCommandParseResult {
    const trimmed = value.trim()
    if (!trimmed.startsWith(COMMAND_PREFIX)) return null
    const body = trimmed.slice(COMMAND_PREFIX.length).trim()
    if (!body) return null
    const [name, ...rest] = body.split(/\s+/)
    if (!name) return null
    return { name, args: rest.join(" ") }
}
