/// slab-mobile entrypoint: loads catalogs + persisted config + language
/// preference, then runs the app under a ProviderScope with the catalogs
/// injected.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'app.dart';
import 'app_providers.dart';
import 'l10n/catalog.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();

  final catalogs = Catalogs(
    en: await SlabCatalog.loadDefault('en-US'),
    zh: await SlabCatalog.loadDefault('zh-CN'),
  );

  final container = ProviderContainer(
    overrides: [catalogsProvider.overrideWithValue(catalogs)],
  );
  await container.read(languagePrefProvider.notifier).load();
  await container.read(connectionConfigProvider.notifier).load();

  runApp(UncontrolledProviderScope(
    container: container,
    child: const SlabApp(),
  ));
}
