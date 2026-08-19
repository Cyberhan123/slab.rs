/// Chat screen: history + live turn projection (ListenableBuilder over the
/// framework-free `ConversationController`), approval banner, composer with
/// send / interrupt, connection-phase chip.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

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
      appBar: AppBar(
        title: Text(widget.sessionName ?? catalog.t('pages.assistant.runtime.newChat')),
        leading: BackButton(onPressed: () => context.go('/sessions')),
        actions: [
          ListenableBuilder(
            listenable: controller,
            builder: (context, _) {
              final state = controller.state;
              final Widget chip;
              switch (state.connection) {
                case ConnectionPhase.connecting:
                  chip = _StatusChip(label: t('mobile.chat.connecting'), active: true);
                case ConnectionPhase.reconnecting:
                  chip = _StatusChip(label: t('mobile.chat.reconnecting'), active: true, warn: true);
                case ConnectionPhase.idle when state.error != null:
                  chip = _StatusChip(label: catalog.t('common.status.error'), active: true, warn: true);
                default:
                  chip = const SizedBox.shrink();
              }
              return chip;
            },
          ),
          const SizedBox(width: 8),
        ],
      ),
      body: SafeArea(
        child: Column(
          children: [
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
                            child: Center(child: CircularProgressIndicator(strokeWidth: 2)),
                          );
                        }
                        return Padding(
                          padding: const EdgeInsets.all(12),
                          child: Text(
                            t('mobile.chat.restoreFailed', {'message': state.error ?? ''}),
                            textAlign: TextAlign.center,
                            style: TextStyle(fontSize: SlabMetrics.textCaption, color: Theme.of(context).colorScheme.error),
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
                            const SizedBox(width: 12, height: 12, child: CircularProgressIndicator(strokeWidth: 1.5)),
                            const SizedBox(width: 8),
                            Text(t('mobile.chat.modelLoading'), style: Theme.of(context).textTheme.bodySmall),
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
    final scheme = Theme.of(context).colorScheme;
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
                child: TextField(
                  controller: composer,
                  minLines: 1,
                  maxLines: 5,
                  textInputAction: TextInputAction.newline,
                  decoration: InputDecoration(hintText: t('mobile.chat.inputHint')),
                  onSubmitted: (_) => onSend(),
                ),
              ),
              const SizedBox(width: 8),
              running
                  ? IconButton.filled(
                      style: IconButton.styleFrom(backgroundColor: scheme.error),
                      tooltip: t('mobile.chat.stop'),
                      onPressed: () => controller.interrupt(),
                      icon: const Icon(Icons.stop),
                    )
                  : IconButton.filled(
                      tooltip: t('mobile.chat.send'),
                      onPressed: onSend,
                      icon: const Icon(Icons.arrow_upward),
                    ),
            ],
          ),
        );
      },
    );
  }
}

class _StatusChip extends StatelessWidget {
  const _StatusChip({required this.label, required this.active, this.warn = false});

  final String label;
  final bool active;
  final bool warn;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    if (!active) return const SizedBox.shrink();
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(SlabMetrics.radiusSm),
        border: Border.all(color: warn ? scheme.error : scheme.outline),
      ),
      child: Text(label, style: TextStyle(fontSize: SlabMetrics.textMicro, color: warn ? scheme.error : scheme.onSurfaceVariant)),
    );
  }
}
