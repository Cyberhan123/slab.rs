/// App shell: TDesign theme from the generated tokens, component-chrome
/// resource delegate, locale wiring, router.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import 'app_providers.dart';
import 'l10n/td_resource_delegate.dart';
import 'routes/app_router.dart';
import 'theme/td_theme.dart';

class SlabApp extends ConsumerWidget {
  const SlabApp({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final td = ref.watch(slabTdThemeProvider);
    return MaterialApp.router(
      title: 'slab',
      debugShowCheckedModeBanner: false,
      theme: buildSlabTdTheme(td, Brightness.light),
      darkTheme: buildSlabTdTheme(td, Brightness.dark),
      themeMode: ThemeMode.system,
      routerConfig: ref.watch(appRouterProvider),
      builder: (context, child) {
        // Component-chrome i18n must be (re)injected after MaterialApp exists
        // and refreshed with the live context on every rebuild.
        final container = ProviderScope.containerOf(context);
        final delegate = SlabResourceDelegate(container);
        setTResourceBuilder(
          (ctx) => delegate..updateContext(ctx),
          needAlwaysBuild: true,
        );
        return child!;
      },
    );
  }
}
