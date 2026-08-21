/// slab-mobile entrypoint: loads catalogs + TDesign theme + persisted config
/// + language preference, then runs the app under a ProviderScope with the
/// catalogs injected.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'app.dart';
import 'app_providers.dart';
import 'l10n/catalog.dart';
import 'theme/td_theme.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();

  final catalogs = Catalogs(
    en: await SlabCatalog.loadDefault('en-US'),
    zh: await SlabCatalog.loadDefault('zh-CN'),
  );

  // tdesign_flutter 1.0 resolves the theme per-context from the MaterialApp
  // theme extensions (light/dark); no global multi-theme bootstrap needed.
  final tdTheme = await loadSlabTheme();

  final container = ProviderContainer(
    overrides: [
      catalogsProvider.overrideWithValue(catalogs),
      slabTdThemeProvider.overrideWithValue(tdTheme),
    ],
  );
  await container.read(languagePrefProvider.notifier).load();
  await container.read(connectionConfigProvider.notifier).load();

  runApp(UncontrolledProviderScope(
    container: container,
    child: const SlabApp(),
  ));
}
