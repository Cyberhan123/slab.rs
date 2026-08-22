/// Error-envelope description: server error bodies may carry an `i18n`
/// payload (`{message: {key: "server.errors.x", params: {...}}}`); when the
/// catalog resolves the key we render the localized message, otherwise the
/// raw envelope message. Port of the desktop `translateServerField`-based
/// error description path (the SSE/OpenAI transport taxonomy is desktop-only
/// — the mobile assistant talks the harness WS exclusively).
library;

import '../../../core/network/slab_api_error.dart';
import '../../../l10n/catalog.dart';

/// Translate a REST error for display.
String describeRestError(SlabRestException error, SlabCatalog catalog) {
  final key = error.i18nKey;
  if (key != null) {
    final args = <String, String>{};
    error.i18nParams?.forEach((name, value) => args[name] = '$value');
    final translated = catalog.t(key, args);
    // SlabCatalog falls back to the key itself when missing.
    if (translated != key) return translated;
  }
  return error.message;
}
