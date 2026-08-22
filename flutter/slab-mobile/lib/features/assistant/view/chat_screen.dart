/// Chat screen: timeline (history markers, compaction dividers, model-load
/// indicator via [MessageList]) over the framework-free conversation
/// controller, approval banner, composer with send / interrupt, token-usage
/// indicator, rollback affordance with a danger confirm.
///
/// The controller is page-owned: one per navigation onto `/chat/:sessionId`
/// (pristine state per session, mirroring the TS hook keyed on sessionId).
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:get_it/get_it.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:go_router/go_router.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import 'package:slab_mobile/core/app/connection_cubit.dart';
import 'package:slab_mobile/data/local/session_meta_dao.dart';
import 'package:slab_mobile/core/di/service_locator.dart';
import 'package:slab_mobile/core/app/locale_cubit.dart';
import 'package:slab_mobile/domain/conversation/conversation_controller.dart';
import 'package:slab_mobile/core/l10n/mobile_strings.dart';
import 'package:slab_mobile/core/theme/slab_tokens.g.dart';
import 'package:slab_mobile/domain/session_labels.dart';
import 'package:slab_mobile/core/network/slab_api_error.dart' show SlabRestException;
import '../commands/request_errors.dart';
import '../models/model_cubit.dart';
import '../models/model_repository.dart';
import '../models/model_status_label.dart';
import 'widgets/approval_banner.dart';
import 'widgets/composer/composer_bar.dart';
import 'widgets/composer/model_switch_dialog.dart';
import 'widgets/messages/message_list.dart';
import 'widgets/messages/token_usage_indicator.dart';

class ChatScreen extends StatefulWidget {
  const ChatScreen({
    super.key,
    required this.sessionId,
    this.sessionName,
    this.controller,
    this.modelCubit,
  });

  final String sessionId;
  final String? sessionName;

  /// Test seam: an inert injected controller keeps the WS stack (and its
  /// uncancellable 5s timeout timer) out of widget tests under FakeAsync.
  final ConversationController? controller;

  /// Test seam: a pre-built model cubit replaces the page-owned one.
  final ModelCubit? modelCubit;

  @override
  State<ChatScreen> createState() => _ChatScreenState();
}

class _ChatScreenState extends State<ChatScreen> {
  final _scroll = ScrollController();
  bool _autoScroll = true;
  late final ConversationController _controller;
  bool _ownsController = false;
  String? _lastActionErrorKey;
  String? _lastPrepareError;

  @override
  void initState() {
    super.initState();
    final injected = widget.controller;
    if (injected != null) {
      _controller = injected;
    } else {
      final config = context.read<ConnectionCubit>().state;
      if (config == null) throw StateError('no connection config');
      _controller = ConversationController(
        sessionId: widget.sessionId,
        baseUrl: config.baseUrl,
      )..start();
      _ownsController = true;
    }
    _controller.addListener(_onControllerChanged);
    // Session pointer for cold-start restore (drift kv; guarded — tests may
    // build the screen against a bare get_it).
    if (GetIt.I.isRegistered<SessionMetaDao>()) {
      unawaited(getIt<SessionMetaDao>().setCurrentSessionId(widget.sessionId));
    }
  }

  void _onControllerChanged() {
    // One-shot action errors (compact/fork failures) surface as toasts.
    final actionError = _controller.state.actionError;
    if (actionError == null) return;
    final key = '${actionError.kind}:${actionError.message}';
    if (key == _lastActionErrorKey) return;
    _lastActionErrorKey = key;
    TToast.showText(actionError.message, context: context);
  }

  @override
  void dispose() {
    _controller.removeListener(_onControllerChanged);
    _scroll.dispose();
    if (_ownsController) _controller.dispose();
    super.dispose();
  }

  void _maybeAutoScroll() {
    if (!_autoScroll || !_scroll.hasClients) return;
    _scroll.animateTo(
      _scroll.position.maxScrollExtent,
      duration: const Duration(milliseconds: 200),
      curve: SlabMetrics.easeOutExpo,
    );
  }

  /// First-prompt auto-title when the server name is still a default label.
  Future<void> _autoTitle(String text) async {
    final serverName = widget.sessionName;
    if (serverName != null && GetIt.I.isRegistered<SessionMetaDao>() && isDefaultSessionLabel(serverName)) {
      await getIt<SessionMetaDao>().upsertLabel(widget.sessionId, createConversationLabel(text, serverName));
    }
  }

  /// New-session action (navbar + switch-dialog "create").
  Future<void> _createSession() async {
    final client = context.read<ConnectionCubit>().client;
    if (client == null) return;
    final record = await client.createSession();
    if (!mounted) return;
    context.go(
      '/chat/${record.id}?name=${Uri.encodeQueryComponent(record.name)}',
    );
  }

  /// Open the model picker sheet; a selection on a populated session goes
  /// through the switch dialog (keep vs new session).
  Future<void> _openModelPicker(BuildContext context) async {
    final cubit = context.read<ModelCubit>();
    final picked = await showModalBottomSheet<String>(
      context: context,
      builder: (sheetContext) => SafeArea(
        child: BlocProvider<ModelCubit>.value(
          value: cubit,
          child: _ModelPickerSheet(cubit: cubit),
        ),
      ),
    );
    if (picked == null || picked == cubit.state.selectedId) return;
    if (_controller.state.messages.isNotEmpty) {
      cubit.requestSwitch(picked);
    } else {
      await cubit.select(picked);
    }
  }

  /// Danger confirm before retracting a turn (desktop parity).
  Future<void> _confirmRollback(int turnIndex) async {
    final localeCubit = context.read<LocaleCubit>();
    final locale = localeCubit.resolvedTag;
    final confirmed = await TDialog.show<bool>(
      context,
      dialog: TDialog(
        title: Text(mobileT(locale, 'mobile.chat.rollbackTitle')),
        content: Text(mobileT(locale, 'mobile.chat.rollbackBody')),
        actions: [
          TDialogAction(
            child: Text(mobileT(locale, 'common.actions.cancel')),
            result: false,
          ),
          TDialogAction(
            child: Text(mobileT(locale, 'pages.assistant.message.rollback')),
            result: true,
            role: TDialogActionRole.destructive,
          ),
        ],
      ),
    );
    if (confirmed == true) {
      await _controller.rollbackFromTurn(turnIndex);
    }
  }

  Widget _buildScreen(BuildContext context) {
    final localeCubit = context.watch<LocaleCubit>();
    final locale = localeCubit.resolvedTag;
    final catalog = localeCubit.catalog;
    String t(String key, [Map<String, String> args = const {}]) =>
        mobileT(locale, key, args);

    return BlocListener<ModelCubit, ModelState>(
      listenWhen: (previous, current) =>
          previous.pendingSwitchTo != current.pendingSwitchTo &&
          current.pendingSwitchTo != null,
      listener: (context, state) =>
          _showSwitchDialog(context, state.pendingSwitchTo!),
      child: Scaffold(
        body: SafeArea(
          bottom: false,
          child: Column(
            children: [
              TNavBar(
                title:
                    widget.sessionName ??
                    catalog.t('pages.assistant.runtime.newChat'),
                useDefaultBack: true,
                onBack: () => context.go('/sessions'),
                actions: [
                  TNavBarItem(
                    customWidget: ListenableBuilder(
                      listenable: _controller,
                      builder: (context, _) {
                        final state = _controller.state;
                        switch (state.connection) {
                          case ConnectionPhase.connecting:
                            return _StatusTag(
                              label: t('mobile.chat.connecting'),
                              colorScheme: TTagColorScheme.primary,
                            );
                          case ConnectionPhase.reconnecting:
                            return _StatusTag(
                              label: t('mobile.chat.reconnecting'),
                              colorScheme: TTagColorScheme.warning,
                            );
                          case ConnectionPhase.idle when state.error != null:
                            return _StatusTag(
                              label: catalog.t('common.status.error'),
                              colorScheme: TTagColorScheme.danger,
                            );
                          default:
                            return const SizedBox.shrink();
                        }
                      },
                    ),
                  ),
                  TNavBarItem(icon: TIcons.add_circle, onTap: _createSession),
                ],
              ),
              // Slim model bar: current model + lifecycle status; tap opens the
              // picker (desktop header-select parity).
              Builder(
                builder: (context) {
                  final cubit = context.watch<ModelCubit>();
                  // One-shot localized toast for prepare failures.
                  final prepareError = cubit.state.prepareError;
                  if (prepareError != null && prepareError != _lastPrepareError) {
                    _lastPrepareError = prepareError;
                    final localeCubit = context.read<LocaleCubit>();
                    WidgetsBinding.instance.addPostFrameCallback((_) {
                      final message = describeRestError(
                        SlabRestException(prepareError, null),
                        localeCubit.catalog,
                      );
                      TToast.showText(message, context: context);
                    });
                  }
                  final controllerState = _controller.state;
                  final label = getSelectedModelStatusLabel(
                    sessionReady: true,
                    isHistoryLoading: controllerState.isHistoryLoading,
                    isCreatingSession: false,
                    isDeletingSession: false,
                    modelLoading: cubit.state.loading,
                    isPreparingModel: cubit.state.preparing,
                    eventsConnected:
                        controllerState.connection == ConnectionPhase.ready,
                    selectedModel: cubit.state.selected,
                    catalog: context.read<LocaleCubit>().catalog,
                  );
                  return InkWell(
                    onTap: () => _openModelPicker(context),
                    child: Padding(
                      padding: const EdgeInsets.fromLTRB(16, 4, 16, 6),
                      child: Row(
                        children: [
                          Icon(
                            TIcons.setting_1,
                            size: 13,
                            color: context.tTheme.textColorSecondary,
                          ),
                          const SizedBox(width: 6),
                          Expanded(
                            child: Text(
                              label,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: TextStyle(
                                fontSize: 11,
                                color: context.tTheme.textColorSecondary,
                              ),
                            ),
                          ),
                          if (cubit.state.preparing) ...[
                            const SizedBox(width: 6),
                            const TLoading(
                              size: TLoadingSize.small,
                              icon: TLoadingIcon.circle,
                            ),
                          ],
                        ],
                      ),
                    ),
                  );
                },
              ),
              Expanded(
                child: NotificationListener<ScrollNotification>(
                  onNotification: (notification) {
                    if (notification is ScrollUpdateNotification &&
                        _scroll.hasClients) {
                      final distance =
                          _scroll.position.maxScrollExtent - _scroll.offset;
                      _autoScroll = distance < 120;
                    }
                    return false;
                  },
                  child: ListenableBuilder(
                    listenable: _controller,
                    builder: (context, _) {
                      final state = _controller.state;
                      WidgetsBinding.instance.addPostFrameCallback(
                        (_) => _maybeAutoScroll(),
                      );
                      final rows = buildScrollerRows(
                        messages: state.messages,
                        compactionMarkers: state.compactionMarkers,
                        historyCount: state.historyCount,
                        sessionLoading: state.isHistoryLoading,
                        modelLoad: state.modelLoad,
                      );
                      if (state.error != null) {
                        rows.add(
                          ErrorRow(
                            message: t('mobile.chat.restoreFailed', {
                              'message': state.error ?? '',
                            }),
                          ),
                        );
                      }
                      return MessageList(
                        rows: rows,
                        locale: locale,
                        catalog: catalog,
                        userMessageTurnIndex: state.userMessageTurnIndex,
                        onRollback: _confirmRollback,
                        scrollController: _scroll,
                      );
                    },
                  ),
                ),
              ),
              ListenableBuilder(
                listenable: _controller,
                builder: (context, _) {
                  final state = _controller.state;
                  return Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      if (state.turnPhase == TurnPhase.modelLoading)
                        Padding(
                          padding: const EdgeInsets.only(
                            bottom: 4,
                            left: 12,
                            right: 12,
                          ),
                          child: Row(
                            children: [
                              const TLoading(
                                size: TLoadingSize.small,
                                icon: TLoadingIcon.circle,
                              ),
                              const SizedBox(width: 8),
                              TText(
                                t('mobile.chat.modelLoading'),
                                style: TextStyle(
                                  fontSize: SlabMetrics.textCaption,
                                  color: context.tTheme.textColorSecondary,
                                ),
                              ),
                            ],
                          ),
                        ),
                      if (state.approvals.isNotEmpty)
                        Padding(
                          padding: const EdgeInsets.fromLTRB(8, 4, 8, 4),
                          child: ApprovalBanner(
                            approvals: state.approvals,
                            onResolve: (request, approved) => _controller
                                .resolveApproval(request.itemId, approved),
                            t: catalog.t,
                            locale: locale,
                          ),
                        ),
                    ],
                  );
                },
              ),
              ListenableBuilder(
                listenable: _controller,
                builder: (context, _) {
                  final state = _controller.state;
                  final usage = state.turnUsage;
                  return Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 12),
                    child: Align(
                      alignment: Alignment.centerLeft,
                      child: usage != null
                          ? TokenUsageIndicator(
                              usage: usage,
                              catalog: context.read<LocaleCubit>().catalog,
                              contextWindowTokens: context
                                  .read<ModelCubit>()
                                  .state
                                  .loadedContextLength,
                            )
                          : const SizedBox.shrink(),
                    ),
                  );
                },
              ),
              ComposerBar(
                controller: _controller,
                sessionId: widget.sessionId,
                locale: locale,
                catalog: catalog,
                onSubmitted: _autoTitle,
              ),
            ],
          ),
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final provided = widget.modelCubit;
    final child = Builder(builder: _buildScreen);
    if (provided != null) {
      return MultiBlocProvider(
        providers: [BlocProvider<ModelCubit>.value(value: provided)],
        child: child,
      );
    }
    final client = context.read<ConnectionCubit>().client;
    if (client == null) throw StateError('no connection client');
    return BlocProvider(
      create: (context) =>
          ModelCubit(repository: ModelRepository(client: client))..load(),
      child: child,
    );
  }

  /// Keep-vs-new-session dialog for a model switch on a populated session.
  Future<void> _showSwitchDialog(BuildContext context, String targetId) async {
    final cubit = context.read<ModelCubit>();
    final target = cubit.state.models
        .where((m) => m.id == targetId)
        .firstOrNull;
    if (target == null) return;
    final catalog = context.read<LocaleCubit>().catalog;
    await TDialog.show(
      context,
      dialog: ModelSwitchDialog(
        catalog: catalog,
        fromLabel:
            cubit.state.selected?.displayName ?? cubit.state.selectedId ?? '',
        toLabel: target.displayName,
        messageCount: _controller.state.messages.length,
        creating: false,
        onKeepSession: () {
          Navigator.of(context).pop();
          cubit.applyPendingSwitch();
        },
        onCreateSession: () async {
          Navigator.of(context).pop();
          await _createSession();
        },
      ),
    );
    // A dismissed dialog (tap outside / cancel action) never resolved —
    // clear the pending switch so the next pick starts clean.
    if (cubit.state.pendingSwitchTo == targetId) {
      cubit.cancelPendingSwitch();
    }
  }
}


class _StatusTag extends StatelessWidget {
  const _StatusTag({required this.label, required this.colorScheme});

  final String label;
  final TTagColorScheme colorScheme;

  @override
  Widget build(BuildContext context) {
    // `isLight` moved from the TTag ctor into the TTagThemeData extension.
    return Theme(
      data: Theme.of(
        context,
      ).copyWith(extensions: [TTagThemeData(isLight: true)]),
      child: TTag(label, size: TTagSize.small, colorScheme: colorScheme),
    );
  }
}

/// Bottom sheet listing the chat-capable models; tapping a row pops with
/// the model id.
class _ModelPickerSheet extends StatelessWidget {
  const _ModelPickerSheet({required this.cubit});

  final ModelCubit cubit;

  @override
  Widget build(BuildContext context) {
    final catalog = context.read<LocaleCubit>().catalog;
    final td = context.tTheme;
    final state = cubit.state;
    return SafeArea(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 12, 16, 4),
            child: Text(
              catalog.t('pages.assistant.modelPicker.groupLabel'),
              style: const TextStyle(fontSize: 13, fontWeight: FontWeight.w600),
            ),
          ),
          if (state.loading)
            const Padding(
              padding: EdgeInsets.all(16),
              child: Center(
                child: TLoading(
                  size: TLoadingSize.small,
                  icon: TLoadingIcon.circle,
                ),
              ),
            ),
          if (!state.loading && state.models.isEmpty)
            Padding(
              padding: const EdgeInsets.all(16),
              child: Text(catalog.t('pages.assistant.modelPicker.emptyLabel')),
            ),
          Flexible(
            child: ListView(
              shrinkWrap: true,
              children: [
                for (final model in state.models)
                  TCell(
                    title: Text(model.displayName),
                    subtitle: Text(
                      model.kind == 'local'
                          ? (model.downloaded
                                ? model.status
                                : catalog.t(
                                    'pages.assistant.status.needsDownload',
                                  ))
                          : catalog.t('pages.assistant.status.cloudModel'),
                      style: TextStyle(
                        fontSize: 11,
                        color: td.textColorPlaceholder,
                      ),
                    ),
                    arrow: false,
                    onTap: () => Navigator.of(context).pop(model.id),
                  ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
