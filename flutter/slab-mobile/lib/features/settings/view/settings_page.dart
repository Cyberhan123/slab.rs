/// Settings tab. Placeholder shell — the ported schema-driven settings
/// engine (slices S1–S5) replaces the body; the route and tab wiring land
/// first so the shell layout is stable underneath.
library;

import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import '../../../core/app/locale_cubit.dart';

class SettingsPage extends StatelessWidget {
  const SettingsPage({super.key});

  @override
  Widget build(BuildContext context) {
    final catalog = context.watch<LocaleCubit>().catalog;
    final title = catalog.t('layouts.sidebar.items.settings');
    return Scaffold(
      body: Column(
        children: [
          TNavBar(title: title, useDefaultBack: false),
          Expanded(child: Center(child: TEmpty(emptyText: title))),
        ],
      ),
    );
  }
}
