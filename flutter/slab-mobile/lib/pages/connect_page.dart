/// First-run connect screen: server base URL (+ optional bearer token),
/// health probe, then persist and continue.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import '../app_providers.dart';
import '../data/connection_config.dart';
import '../data/rest_client.dart';
import '../l10n/mobile_strings.dart';
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
    final td = context.tTheme;
    String t(String key, [Map<String, String> args = const {}]) => mobileT(locale, key, args);
    final probe = _probe;

    return Scaffold(
      body: Column(
        children: [
          TNavBar(title: t('mobile.connect.title'), useDefaultBack: false),
          Expanded(
            child: ListView(
              padding: const EdgeInsets.all(16),
              children: [
                TInput(
                  controller: _url,
                  hintText: kDefaultBaseUrl,
                  label: t('mobile.connect.baseUrl'),
                  inputType: TextInputType.url,
                ),
                const SizedBox(height: 12),
                TInput(
                  controller: _token,
                  label: t('mobile.connect.token'),
                  obscureText: true,
                ),
                const SizedBox(height: 16),
                Row(
                  children: [
                    TButton(
                      variant: TButtonVariant.outline,
                      size: TButtonSize.medium,
                      onPressed: _testing ? null : _test,
                      child: Text(_testing ? t('mobile.connect.testing') : t('mobile.connect.test')),
                    ),
                    const SizedBox(width: 12),
                    if (probe != null)
                      Expanded(
                        child: TText(
                          probe.ok
                              ? t('mobile.connect.ok', {'version': probe.version ?? '?'})
                              : t('mobile.connect.unreachable'),
                          style: TextStyle(
                            fontSize: SlabMetrics.textCaption,
                            color: probe.ok ? td.successNormalColor : td.errorNormalColor,
                          ),
                        ),
                      ),
                  ],
                ),
                const SizedBox(height: 24),
                // `isBlock` is gone in 1.0 — stretch via SizedBox instead.
                SizedBox(
                  width: double.infinity,
                  child: TButton(
                    colorScheme: TButtonColorScheme.primary,
                    size: TButtonSize.large,
                    icon: Icon(TIcons.arrow_right),
                    onPressed: _save,
                    child: Text(t('mobile.connect.save')),
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
