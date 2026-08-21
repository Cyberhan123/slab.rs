import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:slab_mobile/theme/slab_tokens.g.dart';
import 'package:slab_mobile/theme/td_theme.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

void main() {
  // File() (not rootBundle) so the test reads the committed artifact directly
  // without an asset bundle — this is the CI drift surface.
  final themeJson = File('assets/theme/tdesign-theme.json').readAsStringSync();
  final td = TDThemeData.fromJson('slab', themeJson, darkName: 'slabDark');

  test('generated light/dark token sets carry identical keys', () {
    expect(SlabTokensLight.tokenNames, SlabTokensDark.tokenNames);
    // And the modes actually differ (dark is not a copy of light).
    expect(SlabTokensLight.background, isNot(SlabTokensDark.background));
  });

  test('spot-checks against the Figma refs in globals.css', () {
    expect(SlabTokensLight.background, const Color(0xFFF7F9FB));
    expect(SlabTokensDark.background, const Color(0xFF0F1518));
    expect(SlabTokensLight.primary, const Color(0xFF0D9488));
    expect(SlabTokensDark.brandGold, const Color(0xFFF1C27D));
    expect(SlabMetrics.radius, 15.2);
    expect(SlabMetrics.textBody, 13);
  });

  test('tdesign theme asset carries slab values in both modes', () {
    expect(td, isNotNull);
    // Brand anchors: normal == slab primary, at the package's per-mode scale
    // index (light 7 / dark 8). Guards against the built-in TDesign light
    // defaults leaking through a missing key.
    expect(td!.brandNormalColor, SlabTokensLight.primary);
    expect(td.brandColor7, SlabTokensLight.primary);
    expect(td.dark!.brandNormalColor, SlabTokensDark.primary);
    expect(td.dark!.brandColor8, SlabTokensDark.primary);
    // Surfaces follow the page background per mode.
    expect(td.bgColorPage, SlabTokensLight.background);
    expect(td.dark!.bgColorPage, SlabTokensDark.background);
    // Functional scales anchored on slab tokens.
    expect(td.errorNormalColor, SlabTokensLight.destructive);
    expect(td.dark!.errorNormalColor, SlabTokensDark.destructive);
    expect(td.successNormalColor, SlabTokensLight.success);
    expect(td.dark!.successNormalColor, SlabTokensDark.success);
    // Radius adapted to slab's rounded scale.
    expect(td.radiusDefault, 13.2);
    expect(td.radiusSmall, 11.2);
  });

  test('theme builds for both brightnesses and exposes the extensions', () {
    final theme = td!;
    for (final brightness in Brightness.values) {
      final built = buildSlabTdTheme(theme, brightness);
      expect(built.extensions.values.whereType<SlabExtras>().single, isNotNull);
      expect(built.extensions.values.whereType<TDThemeData>().single, isNotNull);
      expect(built.colorScheme.brightness, brightness);
    }
    // Distinct bubble tokens resolve per brightness (chat UI consumes these).
    expect(
      buildSlabTdTheme(theme, Brightness.light).extension<SlabExtras>()!.userBubble,
      isNot(buildSlabTdTheme(theme, Brightness.dark).extension<SlabExtras>()!.userBubble),
    );
    // Dark theme carries the dark TDThemeData (not a light leak).
    final darkTheme = buildSlabTdTheme(theme, Brightness.dark);
    expect(darkTheme.extension<TDThemeData>()!.bgColorPage, SlabTokensDark.background);
  });
}
