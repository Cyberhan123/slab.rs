/// First-run connect screen: server base URL (+ optional bearer token),
/// health probe, then persist and continue. A plain stateful page — the only
/// app wiring it touches is [ConnectionCubit] (save + prefill).
library;

import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:go_router/go_router.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import 'package:slab_mobile/core/app/connection_cubit.dart';
import 'package:slab_mobile/core/app/locale_cubit.dart';
import 'package:slab_mobile/data/local/connection_config.dart';
import 'package:slab_mobile/data/rest/rest_client.dart';
import 'package:slab_mobile/core/l10n/mobile_strings.dart';
import 'package:slab_mobile/core/theme/slab_tokens.g.dart';

class ConnectPage extends StatefulWidget {
  const ConnectPage({super.key});

  @override
  State<ConnectPage> createState() => _ConnectPageState();
}

class _ConnectPageState extends State<ConnectPage> {
  late final TextEditingController _url;
  final TextEditingController _token = TextEditingController();
  HealthStatus? _probe;
  bool _testing = false;

  @override
  void initState() {
    super.initState();
    _url = TextEditingController(
      text: context.read<ConnectionCubit>().state?.baseUrl.toString() ?? kDefaultBaseUrl,
    );
  }

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
    await context.read<ConnectionCubit>().save(
          ConnectionConfig(baseUrl: uri, bearerToken: _token.text.trim()),
        );
    if (mounted) context.go('/sessions');
  }

  @override
  Widget build(BuildContext context) {
    final locale = context.watch<LocaleCubit>().resolvedTag;
    final td = context.tTheme;
    String t(String key, [Map<String, String> args = const {}]) => mobileT(locale, key, args);
    final probe = _probe;

    return Scaffold(
      body: SafeArea(
        bottom: false,
        child: Column(
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
      ),
    );
  }
}
