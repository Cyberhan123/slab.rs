/// Conversation list with health indicator, create/rename/delete, and the
/// setup gate redirect (transport errors are NOT treated as "not initialized"
/// — copies the web `SetupGuard` guard semantics).
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../app_providers.dart';
import '../data/rest_client.dart';
import '../l10n/mobile_strings.dart';
import '../widgets/health_indicator.dart';

class SessionsPage extends ConsumerStatefulWidget {
  const SessionsPage({super.key});

  @override
  ConsumerState<SessionsPage> createState() => _SessionsPageState();
}

class _SessionsPageState extends ConsumerState<SessionsPage> {
  List<SessionRecord>? _sessions;
  Object? _error;
  bool _setupChecked = false;
  Timer? _poll;

  @override
  void initState() {
    super.initState();
    _bootstrap();
    _poll = Timer.periodic(const Duration(seconds: 5), (_) => _refreshHealth());
  }

  @override
  void dispose() {
    _poll?.cancel();
    super.dispose();
  }

  Future<void> _bootstrap() async {
    await _checkSetupGate();
    await _refresh();
  }

  Future<void> _checkSetupGate() async {
    final client = ref.read(restClientProvider);
    if (client == null) return;
    try {
      final setup = await client.getSetupStatus();
      if (!mounted) return;
      if (!setup.initialized) {
        context.go('/setup');
        return;
      }
    } on SlabRestException {
      // Unreachable server is NOT "not initialized" (SetupGuard parity) —
      // the sessions list + health dot surface it instead.
    }
    if (mounted) setState(() => _setupChecked = true);
  }

  Future<void> _refresh() async {
    final client = ref.read(restClientProvider);
    if (client == null) return;
    try {
      final sessions = await client.listSessions();
      if (!mounted) return;
      setState(() {
        _sessions = sessions;
        _error = null;
      });
    } on Object catch (error) {
      if (!mounted) return;
      setState(() => _error = error.toString());
    }
  }

  Future<void> _refreshHealth() => _refresh();

  Future<void> _create() async {
    final client = ref.read(restClientProvider);
    if (client == null) return;
    final record = await client.createSession();
    await _refresh();
    if (!mounted) return;
    context.go('/chat/${record.id}?name=${Uri.encodeQueryComponent(record.name)}');
  }

  Future<void> _rename(SessionRecord record) async {
    final locale = ref.read(localeProvider);
    final controller = TextEditingController(text: record.name);
    final name = await showDialog<String>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(mobileT(locale, 'mobile.sessions.rename')),
        content: TextField(
          controller: controller,
          autofocus: true,
          decoration: InputDecoration(labelText: mobileT(locale, 'mobile.sessions.nameLabel')),
        ),
        actions: [
          TextButton(onPressed: () => Navigator.of(context).pop(), child: Text(mobileT(locale, 'common.actions.cancel'))),
          FilledButton(onPressed: () => Navigator.of(context).pop(controller.text.trim()), child: const Text('OK')),
        ],
      ),
    );
    if (name == null || name.isEmpty || name == record.name) return;
    await ref.read(restClientProvider)?.renameSession(id: record.id, name: name);
    await _refresh();
  }

  Future<void> _delete(SessionRecord record) async {
    final locale = ref.read(localeProvider);
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(mobileT(locale, 'mobile.sessions.delete')),
        content: Text(record.name),
        actions: [
          TextButton(onPressed: () => Navigator.of(context).pop(false), child: Text(mobileT(locale, 'common.actions.cancel'))),
          FilledButton(onPressed: () => Navigator.of(context).pop(true), child: Text(mobileT(locale, 'mobile.sessions.delete'))),
        ],
      ),
    );
    if (confirmed != true) return;
    await ref.read(restClientProvider)?.deleteSession(record.id);
    await _refresh();
  }

  @override
  Widget build(BuildContext context) {
    final locale = ref.watch(localeProvider);
    String t(String key, [Map<String, String> args = const {}]) => mobileT(locale, key, args);
    final sessions = _sessions;
    final reachable = _error == null && sessions != null;

    return Scaffold(
      appBar: AppBar(
        title: Text(t('mobile.sessions.title')),
        actions: [
          Padding(
            padding: const EdgeInsets.only(right: 12),
            child: HealthIndicator(
              online: reachable,
              onlineLabel: t('mobile.sessions.serverOnline'),
              offlineLabel: t('mobile.sessions.serverOffline'),
            ),
          ),
          IconButton(
            tooltip: t('mobile.connect.title'),
            icon: const Icon(Icons.settings_ethernet),
            onPressed: () => context.go('/connect'),
          ),
        ],
      ),
      floatingActionButton: FloatingActionButton.extended(
        onPressed: _create,
        icon: const Icon(Icons.add),
        label: Text(t('mobile.sessions.new')),
      ),
      body: _error != null && sessions == null
          ? Center(child: Text(t('mobile.sessions.serverOffline'), style: Theme.of(context).textTheme.bodySmall))
          : sessions == null
              ? const Center(child: CircularProgressIndicator())
              : RefreshIndicator(
                  onRefresh: _refresh,
                  child: sessions.isEmpty && _setupChecked
                      ? ListView(children: [
                          Padding(
                            padding: const EdgeInsets.all(24),
                            child: Center(child: Text(t('mobile.sessions.empty'), style: Theme.of(context).textTheme.bodySmall)),
                          ),
                        ])
                      : ListView.builder(
                          itemCount: sessions.length,
                          itemBuilder: (context, index) {
                            final record = sessions[index];
                            return ListTile(
                              leading: const Icon(Icons.chat_bubble_outline),
                              title: Text(record.name),
                              subtitle: Text(record.updatedAt, style: const TextStyle(fontSize: 11)),
                              trailing: PopupMenuButton<String>(
                                onSelected: (action) {
                                  if (action == 'rename') _rename(record);
                                  if (action == 'delete') _delete(record);
                                },
                                itemBuilder: (context) => [
                                  PopupMenuItem(value: 'rename', child: Text(t('mobile.sessions.rename'))),
                                  PopupMenuItem(value: 'delete', child: Text(t('mobile.sessions.delete'))),
                                ],
                              ),
                              onTap: () => context.go(
                                '/chat/${record.id}?name=${Uri.encodeQueryComponent(record.name)}',
                              ),
                            );
                          },
                        ),
                ),
    );
  }
}
