/// Flutter-localizations bridge over the generated slab catalogs.
///
/// The catalog keeps its string-key `t(key, args)` API (the `server.*`
/// namespace is looked up with runtime keys from server error envelopes, so
/// typed getters can never cover it); this delegate exposes that same catalog
/// through the `Localizations` machinery so Flutter's locale lifecycle and
/// `flutter_localizations` delegates can hang off the same source.
library;

import 'package:flutter/foundation.dart' show SynchronousFuture;
import 'package:flutter/widgets.dart';

import 'catalog.dart';

/// `Localizations.of<SlabLocalizations>(context)` accessor for the active
/// catalog; widgets translate via `SlabLocalizations.of(context).t(key)`.
class SlabLocalizations {
  const SlabLocalizations(this.catalog);

  final SlabCatalog catalog;

  String t(String key, [Map<String, String> args = const {}]) => catalog.t(key, args);

  static SlabLocalizations of(BuildContext context) =>
      Localizations.of<SlabLocalizations>(context, SlabLocalizations)!;

  static const LocalizationsDelegate<SlabLocalizations> delegate = _SlabLocalizationsDelegate();
}

class _SlabLocalizationsDelegate extends LocalizationsDelegate<SlabLocalizations> {
  const _SlabLocalizationsDelegate();

  @override
  bool isSupported(Locale locale) => true;

  @override
  Future<SlabLocalizations> load(Locale locale) => SynchronousFuture<SlabLocalizations>(
        SlabLocalizations(slabCatalogForTag(SlabCatalog.resolveLocale(locale.toString()))),
      );

  @override
  bool shouldReload(_SlabLocalizationsDelegate old) => false;
}
