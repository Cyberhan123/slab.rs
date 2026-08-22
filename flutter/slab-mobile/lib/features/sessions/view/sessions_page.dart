/// Conversation list tab: health indicator, create/rename/delete, setup-gate
/// redirect listener. Transport errors are NOT treated as "not initialized"
/// (web `SetupGuard` parity) — that lives in the cubit.
library;

import 'package:easy_refresh/easy_refresh.dart';
import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:go_router/go_router.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import 'package:slab_mobile/core/app/connection_cubit.dart';
import 'package:slab_mobile/data/local/session_meta_dao.dart';
import 'package:slab_mobile/core/di/service_locator.dart';
import 'package:slab_mobile/core/app/locale_cubit.dart';
import 'package:slab_mobile/core/widgets/health_indicator.dart';
import 'package:slab_mobile/data/rest/rest_client.dart';
import 'package:slab_mobile/core/l10n/mobile_strings.dart';
import '../sessions_cubit.dart';

class SessionsPage extends StatelessWidget {
  const SessionsPage({super.key, this.cubit});

  /// Test seam: a pre-built cubit replaces the page-owned one (used by the
  /// smoke tests to inject a fake REST client). The provider (and thus the
  /// page) owns closing only the cubit it creates; a provided cubit is
  /// closed by its owner — closing it from a sync `dispose` would leave the
  /// async close dangling under FakeAsync.
  final SessionsCubit? cubit;

  @override
  Widget build(BuildContext context) {
    final provided = cubit;
    return provided == null
        ? BlocProvider(
            create: (context) => SessionsCubit(
              client: context.read<ConnectionCubit>().client,
              sessionMeta: getIt<SessionMetaDao>(),
            )..start(),
            child: const _SessionsView(),
          )
        : BlocProvider.value(
            value: provided..start(),
            child: const _SessionsView(),
          );
  }
}

class _SessionsView extends StatelessWidget {
  const _SessionsView();

  Future<void> _create(BuildContext context) async {
    final cubit = context.read<SessionsCubit>();
    try {
      final record = await cubit.create();
      if (!context.mounted) return;
      context.go(
        '/chat/${record.id}?name=${Uri.encodeQueryComponent(record.name)}',
      );
    } on Object {
      // The cubit's refresh already surfaced transport state; creation
      // failures keep the user on the list (health dot goes red).
    }
  }

  Future<void> _rename(BuildContext context, SessionRecord record) async {
    final cubit = context.read<SessionsCubit>();
    final locale = context.read<LocaleCubit>().resolvedTag;
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
    await cubit.rename(id: record.id, name: name);
  }

  Future<void> _delete(BuildContext context, SessionRecord record) async {
    final cubit = context.read<SessionsCubit>();
    final locale = context.read<LocaleCubit>().resolvedTag;
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
    await cubit.delete(record.id);
  }

  @override
  Widget build(BuildContext context) {
    final locale = context.watch<LocaleCubit>().resolvedTag;
    final td = context.tTheme;
    String t(String key, [Map<String, String> args = const {}]) =>
        mobileT(locale, key, args);

    return BlocListener<SessionsCubit, SessionsState>(
      listenWhen: (previous, current) =>
          !previous.setupRedirect && current.setupRedirect,
      listener: (context, state) => context.go('/setup'),
      child: Scaffold(
        body: Stack(
          fit: StackFit.expand,
          children: [
            BlocBuilder<SessionsCubit, SessionsState>(
              builder: (context, state) {
                final sessions = state.sessions;
                final reachable = state.error == null && sessions != null;
                return Column(
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
                                offlineLabel: t(
                                  'mobile.sessions.serverOffline',
                                ),
                              ),
                            ),
                          ),
                        ),
                        TNavBarItem(
                          icon: TIcons.setting,
                          onTap: () => context.go('/connect?edit=1'),
                        ),
                      ],
                    ),
                    Expanded(
                      child: state.error != null && sessions == null
                          ? TEmpty(
                              emptyText: t('mobile.sessions.serverOffline'),
                            )
                          : sessions == null
                          ? const Center(
                              child: TLoading(
                                size: TLoadingSize.medium,
                                icon: TLoadingIcon.circle,
                              ),
                            )
                          : EasyRefresh(
                              header: TRefreshHeader(),
                              onRefresh: () =>
                                  context.read<SessionsCubit>().refresh(),
                              child: sessions.isEmpty && state.setupChecked
                                  ? ListView(
                                      children: [
                                        Padding(
                                          padding: const EdgeInsets.all(24),
                                          child: Center(
                                            child: TEmpty(
                                              emptyText: t(
                                                'mobile.sessions.empty',
                                              ),
                                            ),
                                          ),
                                        ),
                                      ],
                                    )
                                  : ListView.builder(
                                      itemCount: sessions.length,
                                      itemBuilder: (context, index) {
                                        final record = sessions[index];
                                        final displayLabel =
                                            state.labels[record.id] ??
                                            record.name;
                                        return TSwipeCell(
                                          end: TSwipeCellPanel(
                                            children: [
                                              TSwipeCellAction(
                                                label: t(
                                                  'mobile.sessions.rename',
                                                ),
                                                icon: TIcons.edit,
                                                onPressed: (_) =>
                                                    _rename(context, record),
                                              ),
                                              TSwipeCellAction(
                                                label: t(
                                                  'mobile.sessions.delete',
                                                ),
                                                icon: TIcons.delete,
                                                backgroundColor:
                                                    td.errorNormalColor,
                                                onPressed: (_) =>
                                                    _delete(context, record),
                                              ),
                                            ],
                                          ),
                                          child: TCell(
                                            prefix: Icon(
                                              TIcons.chat_bubble,
                                              color: td.textColorSecondary,
                                            ),
                                            title: Text(displayLabel),
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
                );
              },
            ),
            // TFab 1.0 positions itself (Positioned + optional drag/magnet
            // layer), so it rides a body Stack instead of the Scaffold FAB slot.
            TFab(
              icon: const Icon(TIcons.add),
              onPressed: () => _create(context),
            ),
          ],
        ),
      ),
    );
  }
}
