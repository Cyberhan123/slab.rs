/// Mobile-only chrome strings (connect screen, session list, chat composer).
///
/// Shared strings (approvals, common actions, runtime labels) come from the
/// generated `SlabCatalog` (same source as web/desktop). Keys below exist only
/// on mobile — they deliberately do NOT go into `packages/slab-i18n` because
/// its unused-keys guard would flag catalog entries no web consumer imports.
///
/// The per-locale tables live in sibling files (`mobile_strings_en_us.dart`,
/// `mobile_strings_zh_cn.dart`), mirroring the generated per-locale i18n
/// assets. Keep both key sets in parity — guarded by `catalog_test`.
library;

import 'mobile_strings_en_us.dart';
import 'mobile_strings_zh_cn.dart';

const _strings = <String, Map<String, String>>{
  'en-US': mobileStringsEnUs,
  'zh-CN': mobileStringsZhCn,
};

/// Translate a mobile-only key for `locale`, interpolating `{{var}}`.
String mobileT(String locale, String key, [Map<String, String> args = const {}]) {
  var raw = _strings[locale]?[key] ?? _strings['en-US']?[key] ?? key;
  for (final entry in args.entries) {
    raw = raw.replaceAll('{{${entry.key}}}', entry.value);
  }
  return raw;
}
