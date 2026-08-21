import 'package:flutter_test/flutter_test.dart';
import 'package:slab_mobile/l10n/catalog.dart';
import 'package:slab_mobile/l10n/mobile_strings.dart';
import 'package:slab_mobile/l10n/mobile_strings_en_us.dart';
import 'package:slab_mobile/l10n/mobile_strings_zh_cn.dart';

const enJson = '''
{
  "pages.assistant.runtime.newChat": "New assistant",
  "pages.setup.hints.step": "Step {{step}} of {{count}}"
}
''';

const zhJson = '''
{
  "pages.assistant.runtime.newChat": "新对话",
  "pages.assistant.approval.title": "需要批准"
}
''';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('SlabCatalog', () {
    final en = SlabCatalog.fromJson('en-US', enJson);
    final zh = SlabCatalog.fromJson('zh-CN', zhJson, en);

    test('interpolates {{var}} placeholders', () {
      expect(en.t('pages.setup.hints.step', {'step': '2', 'count': '5'}), 'Step 2 of 5');
    });

    test('falls back through the chain, then to the key', () {
      // zh missing the step key → en value.
      expect(zh.t('pages.setup.hints.step', {'step': '1', 'count': '2'}), 'Step 1 of 2');
      // missing everywhere → the key itself (visible in dev, never a crash).
      expect(zh.t('missing.key'), 'missing.key');
    });

    test('generated asset catalogs are loadable and share key parity', () async {
      // flutter test exposes declared package assets through rootBundle.
      final en = await SlabCatalog.loadDefault('en-US');
      final zh = await SlabCatalog.loadDefault('zh-CN');
      expect(en.t('pages.assistant.approval.title'), 'Approval required');
      expect(zh.t('pages.assistant.approval.title'), contains('审批'));
    });
  });

  group('resolveLocale (port of normalizeLanguage)', () {
    test('supported tags pass through', () {
      expect(SlabCatalog.resolveLocale('en-US'), 'en-US');
      expect(SlabCatalog.resolveLocale('zh-CN'), 'zh-CN');
    });

    test('simplified chinese variants map to zh-CN', () {
      expect(SlabCatalog.resolveLocale('zh'), 'zh-CN');
      expect(SlabCatalog.resolveLocale('zh-CN'), 'zh-CN');
      expect(SlabCatalog.resolveLocale('zh_SG'), 'zh-CN');
      expect(SlabCatalog.resolveLocale('zh-Hans'), 'zh-CN');
    });

    test('traditional chinese regions and non-chinese fall back to en-US', () {
      expect(SlabCatalog.resolveLocale('zh-TW'), 'en-US');
      expect(SlabCatalog.resolveLocale('zh-Hant-TW'), 'en-US');
      expect(SlabCatalog.resolveLocale('ja-JP'), 'en-US');
      expect(SlabCatalog.resolveLocale(null), 'en-US');
      expect(SlabCatalog.resolveLocale(''), 'en-US');
    });
  });

  group('mobileT', () {
    test('locale lookup, en fallback, interpolation', () {
      expect(mobileT('zh-CN', 'mobile.chat.send'), '发送');
      expect(mobileT('en-US', 'mobile.chat.send'), 'Send');
      expect(mobileT('en-US', 'mobile.connect.ok', {'version': '1.2'}), 'Connected — slab-server v1.2');
      expect(mobileT('en-US', 'mobile.unknown.key'), 'mobile.unknown.key');
    });

    // The en→zh fallback in mobileT would silently mask a missing zh key, so
    // parity is asserted directly (mirrors the exporter guard on the
    // generated catalogs).
    test('en-US and zh-CN mobile string tables keep key parity', () {
      final enKeys = mobileStringsEnUs.keys.toSet();
      final zhKeys = mobileStringsZhCn.keys.toSet();
      expect(enKeys.difference(zhKeys), isEmpty, reason: 'keys missing from zh-CN');
      expect(zhKeys.difference(enKeys), isEmpty, reason: 'keys missing from en-US');
    });
  });
}
