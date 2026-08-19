/// First-run connect screen: server base URL (+ optional bearer token),
/// health probe, then persist and continue.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../app_providers.dart';
import '../data/connection_config.dart';
import '../data/rest_client.dart';
import '../l10n/mobile_strings.dart';
import '../theme/slab_theme.dart';
import '../theme/slab_tokens.g.dart';

class ConnectPage extends ConsumerStatefulWidget {
  const ConnectPage({super.key});

  @override
  ConsumerState<ConnectPage> createState() => _ConnectPageState();
}

class _ConnectPageState extends ConsumerState<ConnectPage> {
  late final TextEditingController _url =
      TextEditingController(text: ref.read(connectionConfigProvider)?.baseUrl.toString() ?? kDefaultBaseUrl);
  final TextEditingController _token = TextEditingController();
  HealthStatus? _probe;
  bool _testing = false;

  @override
  void dispose() {
    _url.dispose();
    _token.dispose();
    super.dispose();
  }

  Future<void> _test() async {
    final uri = Uri.tryParse(_url.text.trim());
    if (uri == null || !uri.hasScheme || (uri.scheme != 'http' && uri.scheme != 'https')) {
      setState(() => _probe = const HealthStatus(ok: false));
      return;
    }
    setState(() => _testing = true);
    final client = SlabRestClient(baseUrl: uri, bearerToken: _token.text.trim());
    final status = await client.probeHealth();
    client.dispose();
    if (!mounted) return;
    setState(() {
      _probe = status;
      _testing = false;
    });
  }

  Future<void> _save() async {
    final uri = Uri.tryParse(_url.text.trim());
    if (uri == null || !uri.hasScheme) return;
    await ref.read(connectionConfigProvider.notifier).save(
          ConnectionConfig(baseUrl: uri, bearerToken: _token.text.trim()),
        );
    if (mounted) context.go('/sessions');
  }

  @override
  Widget build(BuildContext context) {
    final locale = ref.watch(localeProvider);
    final scheme = Theme.of(context).colorScheme;
    final extras = slabExtras(context);
    String t(String key, [Map<String, String> args = const {}]) => mobileT(locale, key, args);
    final probe = _probe;

    return Scaffold(
      appBar: AppBar(title: Text(t('mobile.connect.title'))),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          TextField(
            controller: _url,
            autocorrect: false,
            enableSuggestions: false,
            keyboardType: TextInputType.url,
            decoration: InputDecoration(
              labelText: t('mobile.connect.baseUrl'),
              hintText: kDefaultBaseUrl,
            ),
          ),
          const SizedBox(height: 12),
          TextField(
            controller: _token,
            autocorrect: false,
            enableSuggestions: false,
            obscureText: true,
            decoration: InputDecoration(labelText: t('mobile.connect.token')),
          ),
          const SizedBox(height: 16),
          Row(
            children: [
              OutlinedButton(
                onPressed: _testing ? null : _test,
                child: Text(_testing ? t('mobile.connect.testing') : t('mobile.connect.test')),
              ),
              const SizedBox(width: 12),
              if (probe != null)
                Expanded(
                  child: Text(
                    probe.ok
                        ? t('mobile.connect.ok', {'version': probe.version ?? '?'})
                        : t('mobile.connect.unreachable'),
                    style: TextStyle(
                      fontSize: SlabMetrics.textCaption,
                      color: probe.ok ? extras.success : scheme.error,
                    ),
                  ),
                ),
            ],
          ),
          const SizedBox(height: 24),
          FilledButton.icon(
            onPressed: _save,
            icon: const Icon(Icons.arrow_forward),
            label: Text(t('mobile.connect.save')),
          ),
        ],
      ),
    );
  }
}
