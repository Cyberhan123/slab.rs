/// Chat timeline: pure row builder (port of `build-scroller-rows.ts`) plus
/// the rendering widget. Ordering: session-load marker (restoring, empty) →
/// messages with the history-restored marker between restored and live →
/// compaction markers in arrival order → transient model-load marker.
library;

import 'package:flutter/material.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import '../../../../../../conversation/conversation_controller.dart';
import '../../../../../../conversation/turn_items.dart';
import '../../../../../l10n/catalog.dart';
import '../../../../../theme/slab_tokens.g.dart';
import 'message_item.dart';

/// A timeline row: a real message or a synthetic status marker.
sealed class ScrollerRow {
  const ScrollerRow({required this.id});
  final String id;
}

final class SessionLoadMarkerRow extends ScrollerRow {
  const SessionLoadMarkerRow() : super(id: '__session_load_marker__');
}

final class HistoryMarkerRow extends ScrollerRow {
  const HistoryMarkerRow() : super(id: '__history_marker__');
}

final class CompactMarkerRow extends ScrollerRow {
  CompactMarkerRow({required this.marker}) : super(id: marker.id);
  final CompactionMarker marker;
}

final class ModelLoadMarkerRow extends ScrollerRow {
  const ModelLoadMarkerRow({required this.modelLoad}) : super(id: '__model_load_marker__');
  final ModelLoadState modelLoad;
}

final class MessageRow extends ScrollerRow {
  MessageRow({required this.message}) : super(id: message.id);
  final ChatMessage message;
}

/// Terminal restore-failure row appended by the chat screen.
final class ErrorRow extends ScrollerRow {
  ErrorRow({required this.message}) : super(id: '__error_row__');
  final String message;
}

/// Build the timeline row list. [historyCount] is the number of restored
/// (pre-live) messages; the history marker renders after them.
List<ScrollerRow> buildScrollerRows({
  required List<ChatMessage> messages,
  required List<CompactionMarker> compactionMarkers,
  required int historyCount,
  required bool sessionLoading,
  required ModelLoadState? modelLoad,
}) {
  final rows = <ScrollerRow>[];
  if (sessionLoading && messages.isEmpty) {
    rows.add(const SessionLoadMarkerRow());
  }
  final restoredCount = historyCount >= 0 ? historyCount : messages.length;
  for (final (index, message) in messages.indexed) {
    rows.add(MessageRow(message: message));
    if (index == restoredCount - 1 && restoredCount > 0 && index < messages.length - 1) {
      rows.add(const HistoryMarkerRow());
    }
  }
  rows.addAll(compactionMarkers.map((marker) => CompactMarkerRow(marker: marker)));
  if (modelLoad != null) {
    rows.add(ModelLoadMarkerRow(modelLoad: modelLoad));
  }
  return rows;
}

/// Renders the timeline; markers share one separator form so system status
/// reads as an ordered timeline (desktop Marker parity).
class MessageList extends StatelessWidget {
  const MessageList({
    super.key,
    required this.rows,
    required this.locale,
    required this.catalog,
    required this.userMessageTurnIndex,
    required this.onRollback,
    this.scrollController,
  });

  final List<ScrollerRow> rows;
  final String locale;
  final SlabCatalog catalog;
  final Map<String, int> userMessageTurnIndex;
  final void Function(int turnIndex) onRollback;
  final ScrollController? scrollController;

  @override
  Widget build(BuildContext context) {
    final td = context.tTheme;
    final t = catalog.t;
    return ListView.builder(
      controller: scrollController,
      padding: const EdgeInsets.symmetric(vertical: 8),
      itemCount: rows.length,
      itemBuilder: (context, index) {
        final row = rows[index];
        return switch (row) {
          MessageRow() => MessageItem(
              message: row.message,
              locale: locale,
              catalog: catalog,
              canRollback: row.message.fromUser && (userMessageTurnIndex[row.message.id] ?? 0) > 0,
              onRollback: () => onRollback(userMessageTurnIndex[row.message.id]!),
            ),
          SessionLoadMarkerRow() => _marker(context, label: t('pages.assistant.loading.title')),
          HistoryMarkerRow() => _marker(context, label: t('pages.assistant.history.restored')),
          CompactMarkerRow() => _marker(
              context,
              label: switch (row.marker.mode) {
                CompactionMode.auto => row.marker.phase == CompactionPhase.compacting
                    ? t('pages.assistant.compaction.autoCompacting')
                    : t('pages.assistant.compaction.autoCompacted'),
                CompactionMode.manual => row.marker.phase == CompactionPhase.compacting
                    ? t('pages.assistant.compaction.manuallyCompacting')
                    : t('pages.assistant.compaction.manuallyCompacted'),
              },
              active: row.marker.phase == CompactionPhase.compacting,
            ),
          ErrorRow() => Padding(
              padding: const EdgeInsets.all(12),
              child: TText(
                row.message,
                textAlign: TextAlign.center,
                style: TextStyle(fontSize: SlabMetrics.textCaption, color: td.errorNormalColor),
              ),
            ),
          ModelLoadMarkerRow() => _marker(
              context,
              label: row.modelLoad.phase == 'downloading'
                  ? t('pages.assistant.modelLoad.downloading')
                  : t('pages.assistant.modelLoad.loading'),
              active: true,
            ),
        };
      },
    );
  }

  Widget _marker(BuildContext context, {required String label, bool active = false}) {
    final td = context.tTheme;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 6, horizontal: 12),
      child: Row(
        children: [
          Expanded(child: Divider(height: 1, color: td.componentStrokeColor)),
          const SizedBox(width: 8),
          if (active)
            const TLoading(size: TLoadingSize.small, icon: TLoadingIcon.circle)
          else
            Icon(TIcons.check_circle, size: 12, color: td.textColorPlaceholder),
          const SizedBox(width: 4),
          Text(
            label,
            style: TextStyle(fontSize: SlabMetrics.textMicro, color: td.textColorPlaceholder),
          ),
          const SizedBox(width: 8),
          Expanded(child: Divider(height: 1, color: td.componentStrokeColor)),
        ],
      ),
    );
  }
}
