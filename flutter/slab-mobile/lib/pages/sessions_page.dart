/// Conversation list with health indicator, create/rename/delete, and the
/// setup gate redirect (transport errors are NOT treated as "not initialized"
/// — copies the web `SetupGuard` guard semantics).
library;

import 'dart:async';

import 'package:easy_refresh/easy_refresh.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

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
    context.go(
      '/chat/${record.id}?name=${Uri.encodeQueryComponent(record.name)}',
    );
  }

  Future<void> _rename(SessionRecord record) async {
    final locale = ref.read(localeProvider);
    final controller = TextEditingController(text: record.name);
    final name = await TDialog.show<String>(
      context,
      dialog: TDialog(
        title: Text(mobileT(locale, 'mobile.sessions.rename')),
        content: TextField(
          controller: controller,
          decoration: InputDecoration(
            hintText: mobileT(locale, 'mobile.sessions.nameLabel'),
          ),
        ),
        actions: [
          TDialogAction(child: Text(mobileT(locale, 'common.actions.cancel'))),
          TDialogAction(
            child: Text(mobileT(locale, 'mobile.common.confirm')),
            closeOnPressed: false,
            onPressed: () => Navigator.of(context).pop(controller.text.trim()),
          ),
        ],
      ),
    );
    if (name == null || name.isEmpty || name == record.name) return;
    await ref
        .read(restClientProvider)
        ?.renameSession(id: record.id, name: name);
    await _refresh();
  }

  Future<void> _delete(SessionRecord record) async {
    final locale = ref.read(localeProvider);
    final confirmed = await TDialog.show<bool>(
      context,
      dialog: TDialog(
        title: Text(mobileT(locale, 'mobile.sessions.delete')),
        content: Text(record.name),
        actions: [
          TDialogAction(
            child: Text(mobileT(locale, 'common.actions.cancel')),
            result: false,
          ),
          TDialogAction(
            child: Text(mobileT(locale, 'mobile.sessions.delete')),
            result: true,
            role: TDialogActionRole.destructive,
          ),
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
    final td = context.tTheme;
    String t(String key, [Map<String, String> args = const {}]) =>
        mobileT(locale, key, args);
    final sessions = _sessions;
    final reachable = _error == null && sessions != null;

    // TFab 1.0 positions itself (Positioned + optional drag/magnet layer),
    // so it rides a body Stack instead of the Scaffold FAB slot.
    return Scaffold(
      body: Stack(
        fit: StackFit.expand,
        children: [
          Column(
            children: [
              TNavBar(
                title: t('mobile.sessions.title'),
                useDefaultBack: false,
                actions: [
                  TNavBarItem(
                    customWidget: Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 4),
                      child: Center(
                        child: HealthIndicator(
                          online: reachable,
                          onlineLabel: t('mobile.sessions.serverOnline'),
                          offlineLabel: t('mobile.sessions.serverOffline'),
                        ),
                      ),
                    ),
                  ),
                  TNavBarItem(
                    icon: TIcons.setting,
                    onTap: () => context.go('/connect'),
                  ),
                ],
              ),
              Expanded(
                child: _error != null && sessions == null
                    ? TEmpty(emptyText: t('mobile.sessions.serverOffline'))
                    : sessions == null
                    ? Center(
                        child: TLoading(
                          size: TLoadingSize.medium,
                          icon: TLoadingIcon.circle,
                        ),
                      )
                    : EasyRefresh(
                        header: TRefreshHeader(),
                        onRefresh: _refresh,
                        child: sessions.isEmpty && _setupChecked
                            ? ListView(
                                children: [
                                  Padding(
                                    padding: const EdgeInsets.all(24),
                                    child: Center(
                                      child: TEmpty(
                                        emptyText: t('mobile.sessions.empty'),
                                      ),
                                    ),
                                  ),
                                ],
                              )
                            : ListView.builder(
                                itemCount: sessions.length,
                                itemBuilder: (context, index) {
                                  final record = sessions[index];
                                  return TSwipeCell(
                                    end: TSwipeCellPanel(
                                      children: [
                                        TSwipeCellAction(
                                          label: t('mobile.sessions.rename'),
                                          icon: TIcons.edit,
                                          onPressed: (_) => _rename(record),
                                        ),
                                        TSwipeCellAction(
                                          label: t('mobile.sessions.delete'),
                                          icon: TIcons.delete,
                                          backgroundColor: td.errorNormalColor,
                                          onPressed: (_) => _delete(record),
                                        ),
                                      ],
                                    ),
                                    child: TCell(
                                      prefix: Icon(
                                        TIcons.chat_bubble,
                                        color: td.textColorSecondary,
                                      ),
                                      title: Text(record.name),
                                      subtitle: Text(record.updatedAt),
                                      onTap: () => context.go(
                                        '/chat/${record.id}?name=${Uri.encodeQueryComponent(record.name)}',
                                      ),
                                    ),
                                  );
                                },
                              ),
                      ),
              ),
            ],
          ),
          TFab(icon: const Icon(TIcons.add), onPressed: _create),
        ],
      ),
    );
  }
}
