/// Setup gate for an uninitialized slab-server: polls `/v1/setup/status`
/// until `initialized` becomes true, then returns to the sessions list.
///
/// The one-time setup wizard itself is desktop-only; on mobile this screen
/// explains the state and auto-advances. Transport errors keep the "checking"
/// state — they are NOT "not initialized" (`SetupGuard` parity).
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import '../app_providers.dart';
import '../data/rest_client.dart';
import '../l10n/mobile_strings.dart';

class SetupGatePage extends ConsumerStatefulWidget {
  const SetupGatePage({super.key});

  @override
  ConsumerState<SetupGatePage> createState() => _SetupGatePageState();
}

class _SetupGatePageState extends ConsumerState<SetupGatePage> {
  Timer? _poll;
  bool _advanceGuard = false;

  @override
  void initState() {
    super.initState();
    _poll = Timer.periodic(const Duration(seconds: 3), (_) => _check());
    _check();
  }

  @override
  void dispose() {
    _poll?.cancel();
    super.dispose();
  }

  Future<void> _check() async {
    final client = ref.read(restClientProvider);
    if (client == null) return;
    try {
      final setup = await client.getSetupStatus();
      if (setup.initialized && mounted && !_advanceGuard) {
        _advanceGuard = true;
        context.go('/sessions');
      }
    } on SlabRestException {
      // Unreachable — keep polling; the gate only advances on a real
      // `initialized: true` response.
    }
  }

  @override
  Widget build(BuildContext context) {
    final locale = ref.watch(localeProvider);
    String t(String key) => mobileT(locale, key);

    return Scaffold(
      body: Column(
        children: [
          TDNavBar(title: t('mobile.setup.title'), useDefaultBack: false),
          Expanded(
            child: Center(
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 480),
                child: Padding(
                  padding: const EdgeInsets.all(24),
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Row(
                        children: [
                          const TDLoading(size: TDLoadingSize.small, icon: TDLoadingIcon.circle),
                          const SizedBox(width: 12),
                          TDText(t('mobile.setup.checking'), style: const TextStyle(fontSize: 11)),
                        ],
                      ),
                      const SizedBox(height: 16),
                      TDText(t('mobile.setup.description'), style: const TextStyle(fontSize: 13, height: 1.5)),
                      const SizedBox(height: 16),
                      TDButton(
                        text: mobileT(locale, 'common.actions.tryAgain'),
                        icon: TDIcons.refresh,
                        type: TDButtonType.outline,
                        size: TDButtonSize.small,
                        onTap: _check,
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}
