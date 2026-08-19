import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:slab_mobile/theme/slab_theme.dart';
import 'package:slab_mobile/theme/slab_tokens.g.dart';

void main() {
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

  test('theme builds for both brightnesses and exposes SlabExtras', () {
    for (final brightness in Brightness.values) {
      final theme = buildSlabTheme(brightness);
      expect(theme.extensions.values.whereType<SlabExtras>().single, isNotNull);
      expect(theme.colorScheme.brightness, brightness);
      expect(theme.textTheme.bodyMedium!.fontSize, SlabMetrics.textBody);
    }
    // Distinct bubble tokens resolve per brightness (chat UI consumes these).
    expect(
      buildSlabTheme(Brightness.light).extension<SlabExtras>()!.userBubble,
      isNot(buildSlabTheme(Brightness.dark).extension<SlabExtras>()!.userBubble),
    );
  });
}
