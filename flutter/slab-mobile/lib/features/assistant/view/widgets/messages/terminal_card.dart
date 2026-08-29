/// Command-execution card: `$ command` header (cwd + exit status), live
/// streaming output or the finalized shell result. The built-in shell tool
/// reports `SandboxedOutput` JSON (`{stdout, stderr, exit_code, timed_out}`);
/// anything else renders raw. ANSI coloring applies to both paths. Port of
/// `message-tool-command-part.tsx` + `terminal.tsx`.
library;

import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import 'package:slab_mobile/domain/conversation/turn_items.dart';
import 'package:slab_mobile/core/utils/ansi.dart';
import 'package:slab_mobile/core/theme/slab_tokens.g.dart';

/// Parsed `SandboxedOutput` (the shell tool's aggregated output envelope).
class SandboxedOutput {
  const SandboxedOutput({required this.stdoutText, this.stderrText, this.exitCode, this.timedOut = false});
  final String stdoutText;
  final String? stderrText;
  final int? exitCode;
  final bool timedOut;
}

/// Decode the shell tool's `SandboxedOutput` JSON; `null` when the payload is
/// raw text (other tools / legacy items).
SandboxedOutput? decodeSandboxedOutput(String raw) {
  final trimmed = raw.trim();
  if (!trimmed.startsWith('{')) return null;
  Object? decoded;
  try {
    decoded = jsonDecode(trimmed);
  } catch (_) {
    return null;
  }
  if (decoded is! Map<String, Object?>) return null;
  if (!decoded.containsKey('stdout') && !decoded.containsKey('exit_code')) return null;
  return SandboxedOutput(
    stdoutText: decoded['stdout'] is String ? decoded['stdout']! as String : '',
    stderrText: decoded['stderr'] is String && (decoded['stderr']! as String).isNotEmpty ? decoded['stderr']! as String : null,
    exitCode: decoded['exit_code'] is int ? decoded['exit_code']! as int : null,
    timedOut: decoded['timed_out'] is bool ? decoded['timed_out']! as bool : false,
  );
}

/// Renders ANSI-styled terminal text on a dark panel with auto-scroll while
/// streaming (blinking caret approximated by a block cursor).
class TerminalBody extends StatefulWidget {
  const TerminalBody({super.key, required this.text, this.streaming = false});

  final String text;
  final bool streaming;

  @override
  State<TerminalBody> createState() => _TerminalBodyState();
}

class _TerminalBodyState extends State<TerminalBody> {
  final _scroll = ScrollController();

  @override
  void didUpdateWidget(TerminalBody oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.streaming && _scroll.hasClients) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (_scroll.hasClients) {
          _scroll.jumpTo(_scroll.position.maxScrollExtent);
        }
      });
    }
  }

  @override
  void dispose() {
    _scroll.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    // The terminal panel is intentionally dark in both themes (matches the
    // desktop terminal surface); constructed, not literal, per the design guard.
    final panelColor = Color.fromARGB(255, 21, 23, 26);
    final defaultForeground = Color.fromARGB(255, 216, 222, 233);
    final spans = parseAnsi(widget.text);
    return Container(
      constraints: const BoxConstraints(maxHeight: 220),
      margin: const EdgeInsets.only(top: 6),
      padding: const EdgeInsets.all(8),
      decoration: BoxDecoration(color: panelColor, borderRadius: BorderRadius.circular(SlabMetrics.radiusSm)),
      child: SingleChildScrollView(
        controller: _scroll,
        child: Text.rich(
          TextSpan(
            children: [
              for (final span in spans)
                TextSpan(
                  text: span.text,
                  style: TextStyle(
                    color: span.style.foreground ?? defaultForeground,
                    backgroundColor: span.style.background,
                    fontWeight: span.style.bold ? FontWeight.w700 : FontWeight.w400,
                    fontStyle: span.style.italic ? FontStyle.italic : FontStyle.normal,
                    decoration: span.style.underline ? TextDecoration.underline : TextDecoration.none,
                    fontSize: SlabMetrics.textMicro,
                    height: 1.45,
                    fontFamilyFallback: SlabMetrics.fontMono,
                  ),
                ),
              if (widget.streaming)
                TextSpan(text: '▍', style: TextStyle(color: defaultForeground, fontSize: 12)),
            ],
          ),
        ),
      ),
    );
  }
}

/// In-stream command execution card.
class TerminalCard extends StatelessWidget {
  const TerminalCard({super.key, required this.part});

  final ToolUiPart part;

  @override
  Widget build(BuildContext context) {
    final td = context.tTheme;
    final input = part.input;
    final command = input is Map<String, Object?> && input['command'] is String ? input['command']! as String : '';
    final cwd = input is Map<String, Object?> && input['cwd'] is String ? input['cwd']! as String : '';

    final live = part.liveOutput.isNotEmpty;
    final sandboxed = !live && part.output is String ? decodeSandboxedOutput(part.output! as String) : null;
    final bodyText = live
        ? part.liveOutput
        : sandboxed?.stdoutText.isNotEmpty == true
            ? sandboxed!.stdoutText
            : (part.output is String ? part.output! as String : part.errorText ?? '');

    final exitCode = sandboxed?.exitCode;
    final failed = part.failed || (exitCode != null && exitCode != 0);

    return Container(
      margin: const EdgeInsets.symmetric(vertical: 4),
      padding: const EdgeInsets.all(10),
      decoration: BoxDecoration(
        color: td.bgColorSecondaryContainer,
        borderRadius: BorderRadius.circular(SlabMetrics.radiusMd),
        border: Border.all(color: failed ? td.errorNormalColor.withValues(alpha: 0.5) : td.componentStrokeColor),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Icon(TIcons.terminal, size: 14, color: failed ? td.errorNormalColor : td.brandNormalColor),
              const SizedBox(width: 6),
              Expanded(
                child: Text(
                  '\$ $command',
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    fontSize: SlabMetrics.textCaption,
                    fontFamilyFallback: SlabMetrics.fontMono,
                    color: td.textColorPrimary,
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ),
              if (cwd.isNotEmpty) ...[
                const SizedBox(width: 8),
                Flexible(
                  child: Text(
                    cwd,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(fontSize: SlabMetrics.textMicro, color: td.textColorPlaceholder),
                  ),
                ),
              ],
            ],
          ),
          if (sandboxed?.timedOut ?? false)
            Padding(
              padding: const EdgeInsets.only(top: 4),
              child: Text(
                'timed out',
                style: TextStyle(fontSize: SlabMetrics.textMicro, color: td.warningNormalColor),
              ),
            ),
          if (exitCode != null)
            Padding(
              padding: const EdgeInsets.only(top: 4),
              child: Text(
                'exit $exitCode',
                style: TextStyle(
                  fontSize: SlabMetrics.textMicro,
                  color: exitCode == 0 ? td.successNormalColor : td.errorNormalColor,
                  fontFamilyFallback: SlabMetrics.fontMono,
                ),
              ),
            ),
          if (bodyText.trim().isNotEmpty)
            TerminalBody(text: bodyText, streaming: live && part.phase == ToolPhase.running),
          if (sandboxed?.stderrText != null) ...[
            const SizedBox(height: 6),
            Text(
              sandboxed!.stderrText!,
              maxLines: 8,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(
                fontSize: SlabMetrics.textMicro,
                height: 1.4,
                color: td.errorNormalColor,
                fontFamilyFallback: SlabMetrics.fontMono,
              ),
            ),
          ],
        ],
      ),
    );
  }
}
