/// Model-switch confirmation: switching a model on a session that already
/// has messages offers "keep session" vs "create new session". Port of
/// `assistant-model-switch-dialog.tsx` (option cards become stacked rows).
library;

import 'package:flutter/material.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import '../../../../../l10n/catalog.dart';
import '../../../../../theme/slab_tokens.g.dart';

class ModelSwitchDialog extends StatelessWidget {
  const ModelSwitchDialog({
    super.key,
    required this.catalog,
    required this.fromLabel,
    required this.toLabel,
    required this.messageCount,
    required this.creating,
    required this.onKeepSession,
    required this.onCreateSession,
  });

  final SlabCatalog catalog;
  final String fromLabel;
  final String toLabel;
  final int messageCount;

  /// True while the new session is being created (spinner on the button).
  final bool creating;
  final VoidCallback onKeepSession;
  final VoidCallback onCreateSession;

  /// The shared catalog strings embed `<strong>` markup; mobile renders
  /// plain text.
  String _plain(String key, [Map<String, String> args = const {}]) =>
      catalog.t(key, args).replaceAll(RegExp(r'</?[a-zA-Z]+[^>]*>'), '');

  @override
  Widget build(BuildContext context) {
    final td = context.tTheme;
    return TDialog(
      title: Text(_plain('pages.assistant.dialog.title')),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            _plain('pages.assistant.dialog.switchingSummary', {'from': fromLabel, 'to': toLabel}),
            style: TextStyle(fontSize: SlabMetrics.textCaption, height: 1.5, color: td.textColorSecondary),
          ),
          const SizedBox(height: 4),
          Text(
            messageCount == 1
                ? _plain('pages.assistant.dialog.sessionSummary_one', {'label': '', 'count': '$messageCount'})
                    .trim()
                : _plain('pages.assistant.dialog.sessionSummary_other', {'label': '', 'count': '$messageCount'})
                    .trim(),
            style: TextStyle(fontSize: SlabMetrics.textCaption, height: 1.5, color: td.textColorSecondary),
          ),
          const SizedBox(height: 10),
          _option(
            context,
            title: _plain('pages.assistant.dialog.keepTitle'),
            description: _plain('pages.assistant.dialog.keepDescription'),
            onTap: onKeepSession,
          ),
          const SizedBox(height: 8),
          _option(
            context,
            title: _plain('pages.assistant.dialog.createTitle'),
            description: _plain('pages.assistant.dialog.createDescription'),
            onTap: onCreateSession,
            trailing: creating ? const TLoading(size: TLoadingSize.small, icon: TLoadingIcon.circle) : null,
          ),
        ],
      ),
      actions: [
        TDialogAction(child: Text(_plain('common.actions.cancel'))),
      ],
    );
  }

  Widget _option(
    BuildContext context, {
    required String title,
    required String description,
    required VoidCallback onTap,
    Widget? trailing,
  }) {
    final td = context.tTheme;
    return InkWell(
      borderRadius: BorderRadius.circular(SlabMetrics.radiusMd),
      onTap: onTap,
      child: Container(
        padding: const EdgeInsets.all(10),
        decoration: BoxDecoration(
          color: td.bgColorSecondaryContainer,
          borderRadius: BorderRadius.circular(SlabMetrics.radiusMd),
          border: Border.all(color: td.componentStrokeColor),
        ),
        child: Row(
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(title, style: const TextStyle(fontSize: 13, fontWeight: FontWeight.w600)),
                  const SizedBox(height: 2),
                  Text(
                    description,
                    style: TextStyle(fontSize: SlabMetrics.textCaption, height: 1.4, color: td.textColorSecondary),
                  ),
                ],
              ),
            ),
            if (trailing != null) ...[const SizedBox(width: 8), trailing],
          ],
        ),
      ),
    );
  }
}
