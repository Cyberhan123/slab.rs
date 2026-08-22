/// Chat screen: timeline (history markers, compaction dividers, model-load
/// indicator via [MessageList]) over the framework-free conversation
/// controller, approval banner, composer with send / interrupt, token-usage
/// indicator, rollback affordance with a danger confirm.
///
/// The controller is page-owned: one per navigation onto `/chat/:sessionId`
/// (pristine state per session, mirroring the TS hook keyed on sessionId).
library;

import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:go_router/go_router.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import '../../../core/app/connection_cubit.dart';
import '../../../core/app/locale_cubit.dart';
import '../../../conversation/conversation_controller.dart';
import '../../../l10n/mobile_strings.dart';
import '../../../theme/slab_tokens.g.dart';
import 'widgets/approval_banner.dart';
import 'widgets/messages/message_list.dart';
import 'widgets/messages/token_usage_indicator.dart';

class ChatScreen extends StatefulWidget {
  const ChatScreen({super.key, required this.sessionId, this.sessionName, this.controller});

  final String sessionId;
  final String? sessionName;

  /// Test seam: an inert injected controller keeps the WS stack (and its
  /// uncancellable 5s timeout timer) out of widget tests under FakeAsync.
  final ConversationController? controller;

  @override
  State<ChatScreen> createState() => _ChatScreenState();
}

class _ChatScreenState extends State<ChatScreen> {
  final _composer = TextEditingController();
  final _scroll = ScrollController();
  bool _autoScroll = true;
  late final ConversationController _controller;
  bool _ownsController = false;
  String? _lastActionErrorKey;

  @override
  void initState() {
    super.initState();
    final injected = widget.controller;
    if (injected != null) {
      _controller = injected;
    } else {
      final config = context.read<ConnectionCubit>().state;
      if (config == null) throw StateError('no connection config');
      _controller = ConversationController(sessionId: widget.sessionId, baseUrl: config.baseUrl)..start();
      _ownsController = true;
    }
    _controller.addListener(_onControllerChanged);
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
    _composer.dispose();
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

  Future<void> _send() async {
    final text = _composer.text.trim();
    if (text.isEmpty) return;
    _composer.clear();
    await _controller.sendText(text);
  }

  /// Danger confirm before retracting a turn (desktop parity).
  Future<void> _confirmRollback(int turnIndex) async {
    final localeCubit = context.read<LocaleCubit>();
    final locale = localeCubit.state;
    final confirmed = await TDialog.show<bool>(
      context,
      dialog: TDialog(
        title: Text(mobileT(locale, 'mobile.chat.rollbackTitle')),
        content: Text(mobileT(locale, 'mobile.chat.rollbackBody')),
        actions: [
          TDialogAction(child: Text(mobileT(locale, 'common.actions.cancel')), result: false),
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

  @override
  Widget build(BuildContext context) {
    final localeCubit = context.watch<LocaleCubit>();
    final locale = localeCubit.state;
    final catalog = localeCubit.catalog;
    String t(String key, [Map<String, String> args = const {}]) => mobileT(locale, key, args);

    return Scaffold(
      body: SafeArea(
        bottom: false,
        child: Column(
          children: [
            TNavBar(
              title: widget.sessionName ?? catalog.t('pages.assistant.runtime.newChat'),
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
                          return _StatusTag(label: t('mobile.chat.connecting'), colorScheme: TTagColorScheme.primary);
                        case ConnectionPhase.reconnecting:
                          return _StatusTag(label: t('mobile.chat.reconnecting'), colorScheme: TTagColorScheme.warning);
                        case ConnectionPhase.idle when state.error != null:
                          return _StatusTag(label: catalog.t('common.status.error'), colorScheme: TTagColorScheme.danger);
                        default:
                          return const SizedBox.shrink();
                      }
                    },
                  ),
                ),
              ],
            ),
            Expanded(
              child: NotificationListener<ScrollNotification>(
                onNotification: (notification) {
                  if (notification is ScrollUpdateNotification && _scroll.hasClients) {
                    final distance = _scroll.position.maxScrollExtent - _scroll.offset;
                    _autoScroll = distance < 120;
                  }
                  return false;
                },
                child: ListenableBuilder(
                  listenable: _controller,
                  builder: (context, _) {
                    final state = _controller.state;
                    WidgetsBinding.instance.addPostFrameCallback((_) => _maybeAutoScroll());
                    final rows = buildScrollerRows(
                      messages: state.messages,
                      compactionMarkers: state.compactionMarkers,
                      historyCount: state.historyCount,
                      sessionLoading: state.isHistoryLoading,
                      modelLoad: state.modelLoad,
                    );
                    if (state.error != null) {
                      rows.add(ErrorRow(message: t('mobile.chat.restoreFailed', {'message': state.error ?? ''})));
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
                        padding: const EdgeInsets.only(bottom: 4, left: 12, right: 12),
                        child: Row(
                          children: [
                            const TLoading(size: TLoadingSize.small, icon: TLoadingIcon.circle),
                            const SizedBox(width: 8),
                            TText(t('mobile.chat.modelLoading'), style: TextStyle(fontSize: SlabMetrics.textCaption, color: context.tTheme.textColorSecondary)),
                          ],
                        ),
                      ),
                    if (state.approvals.isNotEmpty)
                      Padding(
                        padding: const EdgeInsets.fromLTRB(8, 4, 8, 4),
                        child: ApprovalBanner(
                          approvals: state.approvals,
                          onResolve: (request, approved) => _controller.resolveApproval(request.itemId, approved),
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
                        ? TokenUsageIndicator(usage: usage, catalog: context.read<LocaleCubit>().catalog)
                        : const SizedBox.shrink(),
                  ),
                );
              },
            ),
            _Composer(
              controller: _controller,
              composer: _composer,
              onSend: _send,
              t: t,
            ),
          ],
        ),
      ),
    );
  }
}

class _Composer extends StatelessWidget {
  const _Composer({required this.controller, required this.composer, required this.onSend, required this.t});

  final ConversationController controller;
  final TextEditingController composer;
  final Future<void> Function() onSend;
  final String Function(String key, [Map<String, String> args]) t;

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: controller,
      builder: (context, _) {
        final running = controller.state.turnPhase != TurnPhase.idle;
        return Padding(
          padding: const EdgeInsets.fromLTRB(8, 4, 8, 8),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.end,
            children: [
              Expanded(
                child: TTextarea(
                  controller: composer,
                  minLines: 1,
                  maxLines: 5,
                  hintText: t('mobile.chat.inputHint'),
                  onSubmitted: (_) => onSend(),
                ),
              ),
              const SizedBox(width: 8),
              running
                  ? TButton(
                      icon: Icon(TIcons.stop_circle),
                      size: TButtonSize.small,
                      colorScheme: TButtonColorScheme.danger,
                      onPressed: controller.interrupt,
                    )
                  : TButton(
                      icon: Icon(TIcons.send),
                      size: TButtonSize.small,
                      colorScheme: TButtonColorScheme.primary,
                      onPressed: onSend,
                    ),
            ],
          ),
        );
      },
    );
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
      data: Theme.of(context).copyWith(
        extensions: [TTagThemeData(isLight: true)],
      ),
      child: TTag(label, size: TTagSize.small, colorScheme: colorScheme),
    );
  }
}
