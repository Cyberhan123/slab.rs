/// Root shell: branch content above, TDesign bottom tab bar (icon + text)
/// below. Two tabs — conversations and settings; chat opens as a full-screen
/// route pushed above this shell (no tab bar), WeChat-style.
library;

import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:go_router/go_router.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import 'package:slab_mobile/core/app/locale_cubit.dart';
import 'package:slab_mobile/core/l10n/mobile_strings.dart';

class HomeShell extends StatelessWidget {
  const HomeShell({super.key, required this.shell});

  final StatefulNavigationShell shell;

  @override
  Widget build(BuildContext context) {
    final localeCubit = context.watch<LocaleCubit>();
    final settingsLabel = localeCubit.catalog.t('layouts.sidebar.items.settings');
    return Scaffold(
      body: shell,
      bottomNavigationBar: TTabBar(
        variant: TTabBarVariant.iconText,
        value: shell.currentIndex,
        onChanged: shell.goBranch,
        navigationTabs: [
          TTabBarItemConfig(
            onTap: () => shell.goBranch(0),
            tabText: mobileT(localeCubit.state, 'mobile.sessions.title'),
            unselectedIcon: const Icon(TIcons.chat_bubble),
            selectedIcon: const Icon(TIcons.chat_bubble_filled),
          ),
          TTabBarItemConfig(
            onTap: () => shell.goBranch(1),
            tabText: settingsLabel,
            unselectedIcon: const Icon(TIcons.setting),
            selectedIcon: const Icon(TIcons.setting_filled),
          ),
        ],
      ),
    );
  }
}
