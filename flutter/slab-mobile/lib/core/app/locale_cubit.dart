/// App-wide locale preference: `auto | en-US | zh-CN`, persisted under the
/// web's `slab.ui.language` key (shared_preferences, same string values as
/// web/desktop). Exposes the locale-typed override for MaterialApp plus the
/// resolved tag/catalog so widgets translate with one lookup.
library;

import 'dart:ui' show Locale, PlatformDispatcher;

import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../l10n/catalog.dart';

/// User preference; `auto` follows the platform locale.
enum SlabLanguagePreference { auto, enUs, zhCn }

class LocaleCubit extends Cubit<SlabLanguagePreference> {
  LocaleCubit({Catalogs catalogs = defaultCatalogs})
      : _catalogs = catalogs,
        super(SlabLanguagePreference.auto);

  static const _key = 'slab.ui.language';

  /// Persisted representation (unchanged wire format for existing installs).
  static const _tags = {
    SlabLanguagePreference.auto: 'auto',
    SlabLanguagePreference.enUs: 'en-US',
    SlabLanguagePreference.zhCn: 'zh-CN',
  };

  final Catalogs _catalogs;

  /// Locale override for `MaterialApp.locale`; null under auto so platform
  /// resolution (including OS-level language switches) keeps applying.
  Locale? get localeOverride => switch (state) {
        SlabLanguagePreference.auto => null,
        SlabLanguagePreference.enUs => const Locale('en', 'US'),
        SlabLanguagePreference.zhCn => const Locale('zh', 'CN'),
      };

  /// Always a concrete canonical tag; ports the web `normalizeLanguage`
  /// (zh-* → zh-CN, Traditional Chinese regions → en-US) for auto.
  String get resolvedTag =>
      localeOverride == null
          ? SlabCatalog.resolveLocale(PlatformDispatcher.instance.locale.toString())
          : _tags[state]!;

  Locale get resolvedLocale =>
      resolvedTag == 'zh-CN' ? const Locale('zh', 'CN') : const Locale('en', 'US');

  SlabCatalog get catalog => resolvedTag == 'zh-CN' ? _catalogs.zh : _catalogs.en;

  Future<void> load() async {
    final prefs = await SharedPreferences.getInstance();
    final value = prefs.getString(_key);
    emit(switch (value) {
      'en-US' => SlabLanguagePreference.enUs,
      'zh-CN' => SlabLanguagePreference.zhCn,
      _ => SlabLanguagePreference.auto,
    });
  }

  Future<void> set(SlabLanguagePreference preference) async {
    emit(preference);
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_key, _tags[preference]!);
  }
}
