/// App shell: theme from the generated tokens, locale wiring, router.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'routes/app_router.dart';
import 'theme/slab_theme.dart';

class SlabApp extends ConsumerWidget {
  const SlabApp({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return MaterialApp.router(
      title: 'slab',
      debugShowCheckedModeBanner: false,
      theme: buildSlabTheme(Brightness.light),
      darkTheme: buildSlabTheme(Brightness.dark),
      themeMode: ThemeMode.system,
      routerConfig: ref.watch(appRouterProvider),
    );
  }
}
