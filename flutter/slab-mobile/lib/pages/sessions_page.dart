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
    context.go('/chat/${record.id}?name=${Uri.encodeQueryComponent(record.name)}');
  }

  Future<void> _rename(SessionRecord record) async {
    final locale = ref.read(localeProvider);
    final controller = TextEditingController(text: record.name);
    final name = await showDialog<String>(
      context: context,
      builder: (dialogContext) => TDInputDialog(
        textEditingController: controller,
        title: mobileT(locale, 'mobile.sessions.rename'),
        hintText: mobileT(locale, 'mobile.sessions.nameLabel'),
        leftBtn: TDDialogButtonOptions(
          title: mobileT(locale, 'common.actions.cancel'),
          action: () => Navigator.of(dialogContext).pop(),
          height: 56,
        ),
        rightBtn: TDDialogButtonOptions(
          title: mobileT(locale, 'mobile.common.confirm'),
          action: () => Navigator.of(dialogContext).pop(controller.text.trim()),
          height: 56,
        ),
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
      builder: (dialogContext) => TDAlertDialog(
        title: mobileT(locale, 'mobile.sessions.delete'),
        content: record.name,
        leftBtn: TDDialogButtonOptions(
          title: mobileT(locale, 'common.actions.cancel'),
          action: () => Navigator.of(dialogContext).pop(false),
          height: 56,
        ),
        rightBtn: TDDialogButtonOptions(
          title: mobileT(locale, 'mobile.sessions.delete'),
          titleColor: TDTheme.of(dialogContext).errorNormalColor,
          action: () => Navigator.of(dialogContext).pop(true),
          height: 56,
        ),
      ),
    );
    if (confirmed != true) return;
    await ref.read(restClientProvider)?.deleteSession(record.id);
    await _refresh();
  }

  @override
  Widget build(BuildContext context) {
    final locale = ref.watch(localeProvider);
    final td = TDTheme.of(context);
    String t(String key, [Map<String, String> args = const {}]) => mobileT(locale, key, args);
    final sessions = _sessions;
    final reachable = _error == null && sessions != null;

    return Scaffold(
      floatingActionButton: TDFab(
        icon: const Icon(TDIcons.add),
        onClick: _create,
      ),
      body: Column(
        children: [
          TDNavBar(
            title: t('mobile.sessions.title'),
            useDefaultBack: false,
            rightBarItems: [
              TDNavBarItem(
                iconWidget: Padding(
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
              TDNavBarItem(
                icon: TDIcons.setting,
                action: () => context.go('/connect'),
              ),
            ],
          ),
          Expanded(
            child: _error != null && sessions == null
                ? TDEmpty(emptyText: t('mobile.sessions.serverOffline'))
                : sessions == null
                    ? Center(child: TDLoading(size: TDLoadingSize.medium, icon: TDLoadingIcon.circle))
                    : EasyRefresh(
                        header: TDRefreshHeader(),
                        onRefresh: _refresh,
                        child: sessions.isEmpty && _setupChecked
                            ? ListView(children: [
                                Padding(
                                  padding: const EdgeInsets.all(24),
                                  child: Center(child: TDEmpty(emptyText: t('mobile.sessions.empty'))),
                                ),
                              ])
                            : ListView.builder(
                                itemCount: sessions.length,
                                itemBuilder: (context, index) {
                                  final record = sessions[index];
                                  return TDSwipeCell(
                                    cell: TDCell(
                                      leftIconWidget: Icon(TDIcons.chat_bubble, color: td.textColorSecondary),
                                      title: record.name,
                                      description: record.updatedAt,
                                      onClick: (_) => context.go(
                                        '/chat/${record.id}?name=${Uri.encodeQueryComponent(record.name)}',
                                      ),
                                    ),
                                    right: TDSwipeCellPanel(
                                      children: [
                                        TDSwipeCellAction(
                                          label: t('mobile.sessions.rename'),
                                          icon: TDIcons.edit,
                                          onPressed: (_) => _rename(record),
                                        ),
                                        TDSwipeCellAction(
                                          label: t('mobile.sessions.delete'),
                                          icon: TDIcons.delete,
                                          backgroundColor: td.errorNormalColor,
                                          onPressed: (_) => _delete(record),
                                        ),
                                      ],
                                    ),
                                  );
                                },
                              ),
                      ),
          ),
        ],
      ),
    );
  }
}
