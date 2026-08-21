/// In-stream tool card: name, one-line input summary, phase badge (TTag),
/// live or finalized output. Colors come from the `SlabExtras` tokens
/// (`ai-tool`) plus the TDesign functional scales.
library;

import 'package:flutter/material.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import '../conversation/turn_items.dart';
import '../l10n/mobile_strings.dart';
import '../theme/td_theme.dart';
import '../theme/slab_tokens.g.dart';

class ToolCard extends StatelessWidget {
  const ToolCard({super.key, required this.part, required this.locale});

  final ToolUiPart part;
  final String locale;

  String _inputSummary() {
    final input = part.input;
    if (input is Map<String, Object?>) {
      final command = input['command'];
      if (command is String) return command;
      final query = input['query'];
      if (query is String) return query;
      final changes = input['changes'];
      if (changes is List) return '${changes.length} change(s)';
    }
    if (input is String && input.isNotEmpty) return input;
    return '';
  }

  @override
  Widget build(BuildContext context) {
    final extras = slabExtras(context);
    final td = context.tTheme;
    String t(String key) => mobileT(locale, key);

    final String badge;
    final Color badgeColor;
    switch (part.phase) {
      case ToolPhase.running:
        badge = t('mobile.tool.running');
        badgeColor = td.brandNormalColor;
      case ToolPhase.awaitingApproval:
        badge = t('mobile.tool.awaitingApproval');
        badgeColor = extras.brandGold;
      case ToolPhase.outputAvailable:
        badge = t('mobile.tool.done');
        badgeColor = td.successNormalColor;
      case ToolPhase.outputError:
        badge = t('mobile.tool.failed');
        badgeColor = td.errorNormalColor;
    }

    final output = part.liveOutput.isNotEmpty
        ? part.liveOutput
        : part.errorText ?? (part.output is String ? part.output! as String : '');

    return Container(
      margin: const EdgeInsets.symmetric(vertical: 4),
      padding: const EdgeInsets.all(10),
      decoration: BoxDecoration(
        color: extras.aiTool,
        borderRadius: BorderRadius.circular(SlabMetrics.radiusMd),
        border: Border.all(color: td.componentStrokeColor),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Icon(TIcons.terminal, size: 14, color: extras.aiToolForeground),
              const SizedBox(width: 6),
              Expanded(
                child: TText(
                  '${part.toolName}  ${_inputSummary()}',
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    fontSize: SlabMetrics.textCaption,
                    color: extras.aiToolForeground,
                    fontFamilyFallback: SlabMetrics.fontMono,
                  ),
                ),
              ),
              const SizedBox(width: 8),
              // `isOutline`/`textColor` moved from the TTag ctor into the
              // TTagThemeData extension (per-phase color rides the wrap).
              Theme(
                data: Theme.of(context).copyWith(
                  extensions: [TTagThemeData(isOutline: true, textColor: badgeColor)],
                ),
                child: TTag(badge, size: TTagSize.small),
              ),
            ],
          ),
          if (output.isNotEmpty) ...[
            const SizedBox(height: 8),
            TText(
              output,
              maxLines: 8,
              overflow: TextOverflow.ellipsis,
              style: TextStyle(
                fontSize: SlabMetrics.textMicro,
                height: 1.4,
                fontFamilyFallback: SlabMetrics.fontMono,
              ),
            ),
          ],
        ],
      ),
    );
  }
}
