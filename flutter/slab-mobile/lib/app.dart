/// App shell: TDesign theme from the generated tokens, component-chrome
/// resource delegate, locale wiring, router. App-wide cubits (locale,
/// connection) live in the service locator and are exposed to the tree via
/// BlocProvider.value — their lifecycle belongs to `main()`.
library;

import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:go_router/go_router.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import 'core/app/connection_cubit.dart';
import 'core/app/locale_cubit.dart';
import 'core/di/service_locator.dart';
import 'l10n/td_resource_delegate.dart';
import 'theme/td_theme.dart';

class SlabApp extends StatelessWidget {
  const SlabApp({super.key, required this.router});

  final GoRouter router;

  @override
  Widget build(BuildContext context) {
    final td = getIt<TThemeData>();
    return MultiBlocProvider(
      providers: [
        BlocProvider.value(value: getIt<LocaleCubit>()),
        BlocProvider.value(value: getIt<ConnectionCubit>()),
      ],
      child: MaterialApp.router(
        title: 'slab',
        debugShowCheckedModeBanner: false,
        theme: buildSlabTdTheme(td, Brightness.light),
        darkTheme: buildSlabTdTheme(td, Brightness.dark),
        themeMode: ThemeMode.system,
        routerConfig: router,
        builder: (context, child) {
          // Component-chrome i18n must be (re)injected after MaterialApp exists
          // and refreshed with the live context on every rebuild.
          final delegate = SlabResourceDelegate(getIt<LocaleCubit>());
          setTResourceBuilder(
            (ctx) => delegate..updateContext(ctx),
            needAlwaysBuild: true,
          );
          return child!;
        },
      ),
    );
  }
}
