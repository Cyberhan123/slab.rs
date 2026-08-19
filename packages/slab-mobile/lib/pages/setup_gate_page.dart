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
    final scheme = Theme.of(context).colorScheme;
    String t(String key) => mobileT(locale, key);

    return Scaffold(
      appBar: AppBar(title: Text(t('mobile.setup.title'))),
      body: Center(
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
                    const SizedBox(
                      width: 18,
                      height: 18,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    ),
                    const SizedBox(width: 12),
                    Text(t('mobile.setup.checking'), style: Theme.of(context).textTheme.bodySmall),
                  ],
                ),
                const SizedBox(height: 16),
                Text(t('mobile.setup.description'), style: Theme.of(context).textTheme.bodyMedium),
                const SizedBox(height: 16),
                TextButton.icon(
                  onPressed: _check,
                  icon: Icon(Icons.refresh, size: 16, color: scheme.primary),
                  label: Text(mobileT(locale, 'common.actions.tryAgain')),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
