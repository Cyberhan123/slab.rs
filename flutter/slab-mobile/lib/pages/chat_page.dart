/// Chat screen: history + live turn projection (ListenableBuilder over the
/// framework-free `ConversationController`), approval banner, composer with
/// send / interrupt, connection-phase tag.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import '../app_providers.dart';
import '../conversation/conversation_controller.dart';
import '../l10n/mobile_strings.dart';
import '../theme/slab_tokens.g.dart';
import '../widgets/approval_banner.dart';
import '../widgets/message_bubble.dart';

class ChatPage extends ConsumerStatefulWidget {
  const ChatPage({super.key, required this.sessionId, this.sessionName});

  final String sessionId;
  final String? sessionName;

  @override
  ConsumerState<ChatPage> createState() => _ChatPageState();
}

class _ChatPageState extends ConsumerState<ChatPage> {
  final _composer = TextEditingController();
  final _scroll = ScrollController();
  bool _autoScroll = true;

  @override
  void dispose() {
    _composer.dispose();
    _scroll.dispose();
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

  Future<void> _send(ConversationController controller) async {
    final text = _composer.text.trim();
    if (text.isEmpty) return;
    _composer.clear();
    await controller.sendText(text);
  }

  @override
  Widget build(BuildContext context) {
    final controller = ref.watch(conversationControllerProvider(widget.sessionId));
    final locale = ref.watch(localeProvider);
    final catalog = ref.watch(catalogProvider);
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
                    listenable: controller,
                    builder: (context, _) {
                      final state = controller.state;
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
                  listenable: controller,
                  builder: (context, _) {
                    final state = controller.state;
                    WidgetsBinding.instance.addPostFrameCallback((_) => _maybeAutoScroll());
                    return ListView.builder(
                      controller: _scroll,
                      padding: const EdgeInsets.symmetric(vertical: 8),
                      itemCount: state.messages.length + (state.isHistoryLoading ? 1 : 0) + (state.error != null ? 1 : 0),
                      itemBuilder: (context, index) {
                        final messages = state.messages;
                        if (index < messages.length) {
                          return MessageBubble(message: messages[index], locale: locale);
                        }
                        final offset = index - messages.length;
                        if (offset == 0 && state.isHistoryLoading) {
                          return const Padding(
                            padding: EdgeInsets.all(16),
                            child: Center(child: TLoading(size: TLoadingSize.small, icon: TLoadingIcon.circle)),
                          );
                        }
                        return Padding(
                          padding: const EdgeInsets.all(12),
                          child: TText(
                            t('mobile.chat.restoreFailed', {'message': state.error ?? ''}),
                            textAlign: TextAlign.center,
                            style: TextStyle(fontSize: SlabMetrics.textCaption, color: context.tTheme.errorNormalColor),
                          ),
                        );
                      },
                    );
                  },
                ),
              ),
            ),
            ListenableBuilder(
              listenable: controller,
              builder: (context, _) {
                final state = controller.state;
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
                          onResolve: (request, approved) => controller.resolveApproval(request.itemId, approved),
                          t: catalog.t,
                          locale: locale,
                        ),
                      ),
                  ],
                );
              },
            ),
            _Composer(
              controller: controller,
              composer: _composer,
              onSend: () => _send(controller),
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
