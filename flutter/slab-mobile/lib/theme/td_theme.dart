/// TDesign theme over the generated design tokens.
///
/// The component library (tdesign_flutter) reads its theme from a
/// `TThemeData` ThemeExtension. Values come from
/// `assets/theme/tdesign-theme.json` — the third generated artifact of the
/// one-way pipeline (`bun run gen:mobile` ← globals.css); never hand-edit it.
/// Missing JSON keys would silently fall back to the package's built-in LIGHT
/// palette, so the exporter emits the complete palette for both modes.
///
/// Tokens with no TDesign slot (chat bubbles, tool surfaces, brand gold) ride
/// the `SlabExtras` extension alongside `TThemeData`. Everything else —
/// brand/error/warning/success, surfaces, text, radii — is TDesign theme
/// authority: read it via `context.tTheme`.
library;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart' show rootBundle;
import 'package:tdesign_flutter/tdesign_flutter.dart';

import 'slab_tokens.g.dart';

/// Bespoke tokens with no TDesign theme slot, resolved per brightness
/// (chat bubbles, tool surfaces, brand accents).
@immutable
class SlabExtras extends ThemeExtension<SlabExtras> {
  const SlabExtras({
    required this.aiBubble,
    required this.aiBubbleForeground,
    required this.userBubble,
    required this.userBubbleForeground,
    required this.aiTool,
    required this.aiToolForeground,
    required this.brandGold,
  });

  final Color aiBubble;
  final Color aiBubbleForeground;
  final Color userBubble;
  final Color userBubbleForeground;
  final Color aiTool;
  final Color aiToolForeground;
  final Color brandGold;

  static const light = SlabExtras(
    aiBubble: SlabTokensLight.aiBubble,
    aiBubbleForeground: SlabTokensLight.aiBubbleForeground,
    userBubble: SlabTokensLight.userBubble,
    userBubbleForeground: SlabTokensLight.userBubbleForeground,
    aiTool: SlabTokensLight.aiTool,
    aiToolForeground: SlabTokensLight.aiToolForeground,
    brandGold: SlabTokensLight.brandGold,
  );

  static const dark = SlabExtras(
    aiBubble: SlabTokensDark.aiBubble,
    aiBubbleForeground: SlabTokensDark.aiBubbleForeground,
    userBubble: SlabTokensDark.userBubble,
    userBubbleForeground: SlabTokensDark.userBubbleForeground,
    aiTool: SlabTokensDark.aiTool,
    aiToolForeground: SlabTokensDark.aiToolForeground,
    brandGold: SlabTokensDark.brandGold,
  );

  @override
  SlabExtras copyWith({ThemeExtension<SlabExtras>? other}) => this;

  @override
  SlabExtras lerp(ThemeExtension<SlabExtras>? other, double t) => this;
}

/// The shared "soft-in" curve (`--ease-out-expo`, also the default transition
/// timing in globals.css).
final Cubic slabEaseOutExpo = SlabMetrics.easeOutExpo;
const int slabTransitionMs = 180; // --default-transition-duration

/// Loads the generated TDesign theme asset (`slab` light + `slabDark` dark).
/// Falls back to the package default so a malformed asset cannot brick boot.
Future<TThemeData> loadSlabTheme() async {
  const assetPath = 'assets/theme/tdesign-theme.json';
  try {
    final json = await rootBundle.loadString(assetPath);
    final theme = TThemeData.fromJson('slab', json, darkName: 'slabDark');
    if (theme != null) return theme;
    FlutterError.reportError(
      FlutterErrorDetails(exception: StateError('TThemeData.fromJson returned null for $assetPath')),
    );
  } on FlutterError catch (error) {
    FlutterError.reportError(FlutterErrorDetails(exception: error, library: 'td_theme', context: ErrorDescription('while loading $assetPath')));
  }
  return TThemeData.defaultData();
}

/// MaterialApp `theme`/`darkTheme` for one brightness: the package-built
/// ThemeData (`TThemeBuilder` — colorScheme/scaffold from the tokens, with
/// `TThemeData` injected as extension) plus our `SlabExtras` and slab's font
/// stacks. No Material component themes — the component library is TDesign now.
ThemeData buildSlabTdTheme(TThemeData td, Brightness brightness) {
  final isLight = brightness == Brightness.light;
  final data = isLight ? td : (td.dark ?? td);
  final base = isLight ? TThemeBuilder.light(td) : TThemeBuilder.dark(td);
  return base.copyWith(
    extensions: {data, isLight ? SlabExtras.light : SlabExtras.dark},
    textTheme: base.textTheme.apply(
      fontFamilyFallback: SlabMetrics.fontSans,
      bodyColor: data.textColorPrimary,
      displayColor: data.textColorPrimary,
    ),
    splashFactory: InkSparkle.splashFactory,
  );
}

/// `Theme.of(context).extension<SlabExtras>()!` convenience.
SlabExtras slabExtras(BuildContext context) => Theme.of(context).extension<SlabExtras>()!;
