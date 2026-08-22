/// Pure model-status label machine — priority-ordered, port of the desktop
/// `getSelectedModelStatusLabel`. Returns the composed navbar subtitle.
library;

import '../../../data/model_types.dart';
import '../../../l10n/catalog.dart';

String getSelectedModelStatusLabel({
  required bool sessionReady,
  required bool isHistoryLoading,
  required bool isCreatingSession,
  required bool isDeletingSession,
  required bool modelLoading,
  required bool isPreparingModel,
  required bool eventsConnected,
  AiModelRecord? selectedModel,
  required SlabCatalog catalog,
}) {
  final t = catalog.t;
  if (!sessionReady) return t('pages.assistant.status.preparingSession');
  if (isHistoryLoading) return t('pages.assistant.status.loadingSessionHistory');
  if (isCreatingSession) return t('pages.assistant.status.creatingSession');
  if (isDeletingSession) return t('pages.assistant.status.deletingSession');
  if (modelLoading) return t('pages.assistant.status.loadingModels');

  final model = selectedModel;
  if (model == null) return t('common.fields.selectModel');

  final parts = <String>[model.displayName];
  final runtimeContext = model.runtimeContextLength;
  if (runtimeContext != null && runtimeContext > 0) {
    parts.add(t('pages.assistant.status.runtimeContextWindow', {'formatted': _formatNumber(runtimeContext)}));
  } else if (model.contextWindow != null && model.contextWindow! > 0) {
    parts.add(t('pages.assistant.status.contextWindow', {'formatted': _formatNumber(model.contextWindow!)}));
  } else if (model.pending) {
    parts.add(t('pages.assistant.status.downloading'));
  } else if (model.kind == 'local' && !model.downloaded) {
    parts.add(t('pages.assistant.status.needsDownload'));
  } else if (isPreparingModel) {
    parts.add(t('pages.assistant.status.preparing'));
  } else if (model.kind == 'cloud') {
    parts.add(t('pages.assistant.status.cloudModel'));
  }

  if (eventsConnected) {
    parts.add(t('pages.assistant.connection.connected'));
  }
  return parts.join(' / ');
}

String _formatNumber(int value) {
  // Grouped decimal formatting (Intl.NumberFormat parity) without the
  // flutter_localizations dependency.
  final digits = value.toString();
  final buffer = StringBuffer();
  var count = 0;
  for (var i = digits.length - 1; i >= 0; i--) {
    buffer.write(digits[i]);
    count += 1;
    if (count % 3 == 0 && i > 0) buffer.write(',');
  }
  return buffer.toString().split('').reversed.join();
}
