import type { CommandInfo } from "./harness/types"

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

/**
 * Resolve a composer submission into a dispatch decision, driven by the
 * registry snapshot from `command/list`. A `control` result runs a host action
 * (never reaches the model); `togglePlan` flips the client-side InteractionMode
 * toggle (the `/plan` command — never reaches the model); `send` falls through
 * to `sendMessage` (skills and anything not recognized as a command).
 *
 * Local resolution (no server round-trip) per the "客户端解析" guidance: the
 * client already holds the full command list, so `parseAssistantCommand` +
 * name/alias lookup is authoritative.
 */
export type CommandDispatch =
    | { action: "control"; controlAction: "compact" | "fork" }
    | { action: "togglePlan" }
    | { action: "send" }

export function resolveCommandDispatch(
    value: string,
    commands: CommandInfo[],
): CommandDispatch {
    const parsed = parseAssistantCommand(value)
    if (parsed) {
        const cmd = commands.find(
            (c) => c.name === parsed.name || c.aliases.includes(parsed.name),
        )
        if (cmd?.kind === "control") {
            if (cmd.controlAction === "compact") {
                return { action: "control", controlAction: "compact" }
            }
            if (cmd.controlAction === "fork") {
                return { action: "control", controlAction: "fork" }
            }
        }
        // `/plan` is registered as a Prompt command but the client repurposes it
        // to toggle Plan interaction mode (no message sent); the server stays
        // the source of truth via the `turn/start` interactionMode field.
        if (cmd?.name === "plan") {
            return { action: "togglePlan" }
        }
    }
    return { action: "send" }
}
