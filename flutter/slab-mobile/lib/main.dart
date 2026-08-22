/// slab-mobile entrypoint: loads catalogs + TDesign theme, configures the
/// service locator (which also restores the persisted language preference and
/// connection config so the router redirect runs against real state), then
/// runs the app.
library;

import 'package:flutter/material.dart';
import 'package:flutter_screenutil/flutter_screenutil.dart';

import 'app.dart';
import 'package:slab_mobile/core/app/connection_cubit.dart';
import 'package:slab_mobile/core/di/service_locator.dart';
import 'package:slab_mobile/core/l10n/catalog.dart';
import 'package:slab_mobile/routes/app_router.dart';
import 'package:slab_mobile/core/theme/td_theme.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();

  final catalogs = Catalogs(
    en: await SlabCatalog.loadDefault('en-US'),
    zh: await SlabCatalog.loadDefault('zh-CN'),
  );

  // tdesign_flutter 1.0 resolves the theme per-context from the MaterialApp
  // theme extensions (light/dark); no global multi-theme bootstrap needed.
  final tdTheme = await loadSlabTheme();

  await configureDependencies(catalogs: catalogs, tdTheme: tdTheme);

  runApp(
    ScreenUtilInit(
      // Design-space baseline for the ported screens; extension-based usage
      // (.w/.sp) is opt-in per widget, legacy screens are unaffected.
      designSize: const Size(375, 812),
      minTextAdapt: true,
      builder: (_, _) => SlabApp(router: buildAppRouter(connection: getIt<ConnectionCubit>())),
    ),
  );
}
