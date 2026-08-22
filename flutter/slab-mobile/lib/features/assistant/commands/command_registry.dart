/// Slash-command resolution over the `command/list` registry snapshot.
/// Pure port of the desktop `assistant-commands.ts`: local resolution — the
/// client holds the full command list, so name/alias lookup is authoritative.
library;

import '../../../proto/harness_types.dart' as proto;

class ParsedCommand {
  const ParsedCommand({required this.name, required this.args});
  final String name;
  final String args;
}

/// Parse a leading `/<name>` command. Null when the input is not a command
/// (no leading slash, or a bare slash). First token = name; the rest (joined
/// by single spaces) = args.
ParsedCommand? parseAssistantCommand(String value) {
  final trimmed = value.trim();
  if (!trimmed.startsWith('/')) return null;
  final body = trimmed.substring(1).trim();
  if (body.isEmpty) return null;
  final tokens = body.split(RegExp(r'\s+'));
  final name = tokens.first;
  if (name.isEmpty) return null;
  return ParsedCommand(name: name, args: tokens.skip(1).join(' '));
}

sealed class CommandDispatch {
  const CommandDispatch();
}

/// Runs a host action (compact / fork) — never reaches the model.
final class ControlDispatch extends CommandDispatch {
  const ControlDispatch(this.controlAction);
  final String controlAction; // "compact" | "fork"
}

/// Flips the client-side plan-mode toggle (`/plan`) — no message sent.
final class TogglePlanDispatch extends CommandDispatch {
  const TogglePlanDispatch();
}

/// Falls through to send (skills and anything not recognized as a command).
final class SendDispatch extends CommandDispatch {
  const SendDispatch();
}

/// Resolve a composer submission into a dispatch decision.
CommandDispatch resolveCommandDispatch(String value, List<proto.CommandInfo> commands) {
  final parsed = parseAssistantCommand(value);
  if (parsed != null) {
    proto.CommandInfo? command;
    for (final candidate in commands) {
      if (candidate.name == parsed.name || candidate.aliases.contains(parsed.name)) {
        command = candidate;
        break;
      }
    }
    if (command?.kind == proto.CommandKind.control) {
      if (command!.controlAction == 'compact') return ControlDispatch('compact');
      if (command.controlAction == 'fork') return ControlDispatch('fork');
    }
    // `/plan` is registered as a prompt command but the client repurposes it
    // to toggle plan mode; the next turn/start carries `agentType: "plan"`.
    if (command?.name == 'plan') return const TogglePlanDispatch();
  }
  return const SendDispatch();
}
