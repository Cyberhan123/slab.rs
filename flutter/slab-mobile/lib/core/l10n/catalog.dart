/// Locale catalog over the generated flat JSON (`assets/i18n/*.json`,
/// exported from `packages/slab-i18n` TS catalogs by `bun run gen:mobile`).
///
/// `t(key, args)` keeps i18next `{{var}}` interpolation; language resolution
/// ports `normalizeLanguage` from `packages/slab-i18n/src/index.ts`
/// (zh-* → zh-CN, Traditional Chinese regions → en-US fallback).
library;

import 'dart:convert';

import 'package:flutter/services.dart' show AssetBundle, rootBundle;

/// Both catalogs, loaded in `main()` before `runApp` and registered in the
/// service locator (en-US doubles as the fallback chain root).
class Catalogs {
  const Catalogs({required this.en, required this.zh});
  final SlabCatalog en;
  final SlabCatalog zh;
}

class SlabCatalog {
  SlabCatalog._(this.locale, this._entries, this._fallback);
  final String locale;
  final Map<String, String> _entries;
  final SlabCatalog? _fallback;

  /// Parse from raw JSON text (tests).
  static SlabCatalog fromJson(String locale, String json, [SlabCatalog? fallback]) {
    final decoded = jsonDecode(json);
    final entries = <String, String>{};
    if (decoded is Map<String, Object?>) {
      decoded.forEach((key, value) {
        if (value is String) entries[key] = value;
      });
    }
    return SlabCatalog._(locale, entries, fallback);
  }

  /// Load an asset bundle catalog; en-US is the fallback chain root.
  static Future<SlabCatalog> load(AssetBundle bundle, String locale) async {
    final en = SlabCatalog.fromJson('en-US', await bundle.loadString('assets/i18n/en-US.json'));
    if (locale == 'en-US') return en;
    final zh = SlabCatalog.fromJson(locale, await bundle.loadString('assets/i18n/$locale.json'), en);
    return zh;
  }

  static Future<SlabCatalog> loadDefault(String locale) => load(rootBundle, locale);

  /// Resolve a platform locale to a supported one (port of `normalizeLanguage`).
  static String resolveLocale(String? platformLocale) {
    if (platformLocale == 'en-US' || platformLocale == 'zh-CN') return platformLocale!;
    final normalized = (platformLocale ?? '').toLowerCase();
    final chinese = RegExp(r'^zh(?:[-_][a-z]+)*$');
    final traditional = RegExp(r'^zh(?:[-_][a-z]+)*[-_](?:tw|hk|mo|hant)(?:[-_][a-z]+)*');
    if (!chinese.hasMatch(normalized)) return 'en-US';
    if (traditional.hasMatch(normalized)) return 'en-US';
    return 'zh-CN';
  }

  /// Translate with `{{var}}` interpolation; falls back through the chain and
  /// finally to the key itself (visible in dev, never a crash).
  String t(String key, [Map<String, String> args = const {}]) {
    var raw = _entries[key] ?? _fallback?._entries[key] ?? key;
    for (final entry in args.entries) {
      raw = raw.replaceAll('{{${entry.key}}}', entry.value);
    }
    return raw;
  }
}
