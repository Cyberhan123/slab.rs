/// Committed-artifact parity guards for the generated ARB interchange files
/// (reads the checked-in files directly — this is the CI drift surface, same
/// pattern as `test/core/theme/tokens_test.dart`) plus unit tests for the
/// const-map catalogs and the `SlabLocalizations` delegate.
library;

import 'dart:convert';
import 'dart:io';

import 'package:flutter/widgets.dart' show Locale;
import 'package:flutter_test/flutter_test.dart';
import 'package:slab_mobile/core/l10n/catalog.dart';
import 'package:slab_mobile/core/l10n/slab_localizations.dart';

final _placeholder = RegExp(r'\{\{\s*([^}]+?)\s*\}\}');

Map<String, String> _loadArb(String path) {
  final doc = jsonDecode(File(path).readAsStringSync()) as Map<String, dynamic>;
  return {
    for (final entry in doc.entries)
      if (!entry.key.startsWith('@') && entry.value is String) entry.key: entry.value as String,
  };
}

void main() {
  final en = _loadArb('lib/core/l10n/arb/app_en.arb');
  final zh = _loadArb('lib/core/l10n/arb/app_zh.arb');

  test('committed ARB files carry a non-trivial catalog', () {
    expect(en.length, greaterThan(1000));
    expect(en.containsKey('server.errors.notFound'), isTrue,
        reason: 'the runtime server.* namespace must ship');
  });

  test('en/zh ARB keep key parity (both directions)', () {
    final onlyEn = en.keys.toSet().difference(zh.keys.toSet());
    final onlyZh = zh.keys.toSet().difference(en.keys.toSet());
    expect(onlyEn, isEmpty, reason: 'keys missing from zh: $onlyEn');
    expect(onlyZh, isEmpty, reason: 'keys missing from en: $onlyZh');
  });

  test('en/zh ARB keep {{var}} placeholder parity per key', () {
    final mismatched = [
      for (final key in en.keys)
        if (_allMatches(en[key]).join('|') != _allMatches(zh[key]).join('|')) key,
    ];
    expect(mismatched, isEmpty,
        reason: 'placeholder multisets differ for: ${mismatched.take(5).toList()}');
  });

  test('const-map catalogs match the ARB source and interpolate', () {
    // The Dart runtime table and the ARB interchange file must agree verbatim.
    final probe = slabCatalogForTag('en-US');
    expect(probe, same(slabCatalogEn));
    for (final key in en.keys.take(200)) {
      expect(slabCatalogEn.t(key), en[key], reason: 'const map drift at $key');
    }
    expect(
      slabCatalogEn.t('pages.setup.hints.step', {'step': '2', 'count': '5'}),
      contains('2'),
    );
    expect(
      slabCatalogZh.t('pages.setup.hints.step', {'step': '2', 'count': '5'}),
      contains('2'),
    );
  });

  test('SlabLocalizations delegate resolves zh and non-zh locales', () async {
    final zh = await SlabLocalizations.delegate.load(const Locale('zh', 'CN'));
    expect(zh.t('common.actions.cancel'), isNot(equals('common.actions.cancel')));
    // Traditional Chinese regions fall back to en (normalizeLanguage port).
    final tw = await SlabLocalizations.delegate.load(const Locale('zh', 'TW'));
    expect(tw.catalog.locale, 'en-US');
    final ja = await SlabLocalizations.delegate.load(const Locale('ja'));
    expect(ja.catalog.locale, 'en-US');
  });
}

List<String> _allMatches(String? value) =>
    _placeholder.allMatches(value ?? '').map((m) => m.group(1)!).toList()..sort();
