/// Token usage indicator: "Used X%" of the context window (when the runtime
/// context length is known) with an expandable prompt/completion/cached
/// breakdown. Hidden until the first completed turn reports usage.
library;

import 'package:flutter/material.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import '../../../../../../proto/harness_types.dart' as proto;
import '../../../../../l10n/catalog.dart';

class TokenUsageIndicator extends StatelessWidget {
  const TokenUsageIndicator({super.key, required this.usage, required this.catalog, this.contextWindowTokens});

  final proto.TurnUsage usage;
  final SlabCatalog catalog;

  /// Runtime context length of the loaded model; without it the percentage
  /// is skipped and only the token count renders.
  final int? contextWindowTokens;

  String _format(int tokens) {
    if (tokens >= 1000000) return '${(tokens / 1000000).toStringAsFixed(1)}M';
    if (tokens >= 1000) return '${(tokens / 1000).toStringAsFixed(1)}k';
    return '$tokens';
  }

  @override
  Widget build(BuildContext context) {
    final t = catalog.t;
    final window = contextWindowTokens;
    final percent = window != null && window > 0 ? (usage.promptTokens * 100 / window).round() : null;
    final hot = percent != null && percent >= 80;

    return Theme(
      // Light-tag styling rides the TTagThemeData extension (TDesign 1.0).
      data: Theme.of(context).copyWith(extensions: [TTagThemeData(isLight: true)]),
      child: TTag(
        '${t('pages.assistant.usage.total', {'formatted': _format(usage.totalTokens)})}'
        '${percent != null ? ' · ${t('pages.assistant.usage.used', {'percent': '$percent'})}' : ''}'
        '${usage.estimated ? ' ~' : ''}',
        size: TTagSize.small,
        colorScheme: hot ? TTagColorScheme.warning : TTagColorScheme.defaultTheme,
      ),
    );
  }
}
