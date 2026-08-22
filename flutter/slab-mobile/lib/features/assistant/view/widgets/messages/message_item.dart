/// One timeline message: user messages animate in (slide-up, desktop
/// `message-animations` default preset) and carry a rollback affordance on
/// retractable turns; every message gets a copy-text action. The bubble
/// chrome itself stays in [MessageBubble].
library;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import '../../../../../../conversation/turn_items.dart';
import '../../../../../l10n/catalog.dart';
import '../../../../../l10n/mobile_strings.dart';
import '../../../../../theme/slab_tokens.g.dart';
import 'message_bubble.dart';

class MessageItem extends StatefulWidget {
  const MessageItem({
    super.key,
    required this.message,
    required this.locale,
    required this.catalog,
    this.canRollback = false,
    this.onRollback,
  });

  final ChatMessage message;
  final String locale;
  final SlabCatalog catalog;

  /// True when the user message sits at a turn the thread can retract.
  final bool canRollback;
  final VoidCallback? onRollback;

  @override
  State<MessageItem> createState() => _MessageItemState();
}

class _MessageItemState extends State<MessageItem> with SingleTickerProviderStateMixin {
  late final AnimationController _entry = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 220),
  );
  late final Animation<Offset> _slide = Tween(
    begin: const Offset(0, 0.25),
    end: Offset.zero,
  ).animate(CurvedAnimation(parent: _entry, curve: Curves.easeOutCubic));

  @override
  void initState() {
    super.initState();
    // Only user messages animate in; assistant content streams (no entry pop).
    if (widget.message.fromUser) {
      _entry.forward();
    } else {
      _entry.value = 1;
    }
  }

  Future<void> _copyText() async {
    final text = widget.message.parts
        .whereType<TextUiPart>()
        .map((part) => part.text)
        .join('\n');
    if (text.isEmpty) return;
    await Clipboard.setData(ClipboardData(text: text));
    if (!mounted) return;
    TToast.showText(mobileT(widget.locale, 'mobile.chat.copied'), context: context);
  }

  @override
  void dispose() {
    _entry.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final td = context.tTheme;
    final message = widget.message;
    return SlideTransition(
      position: _slide,
      child: FadeTransition(
        opacity: _entry,
        child: Column(
          crossAxisAlignment: message.fromUser ? CrossAxisAlignment.end : CrossAxisAlignment.start,
          children: [
            MessageBubble(message: message, locale: widget.locale, catalog: widget.catalog),
            // Footer actions: rollback (user turns > 0) + copy.
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 1),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  if (widget.canRollback && widget.onRollback != null)
                    _action(
                      context,
                      icon: TIcons.rollback,
                      label: widget.catalog.t('pages.assistant.message.rollback'),
                      color: td.textColorPlaceholder,
                      onTap: widget.onRollback!,
                    ),
                  _action(
                    context,
                    icon: TIcons.copy,
                    label: '',
                    color: td.textColorPlaceholder,
                    onTap: _copyText,
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _action(
    BuildContext context, {
    required IconData icon,
    required String label,
    required Color color,
    required VoidCallback onTap,
  }) {
    return InkWell(
      borderRadius: BorderRadius.circular(SlabMetrics.radiusSm),
      onTap: onTap,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 2),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(icon, size: 13, color: color),
            if (label.isNotEmpty) ...[
              const SizedBox(width: 3),
              Text(
                label,
                style: TextStyle(fontSize: SlabMetrics.textMicro, color: color),
              ),
            ],
          ],
        ),
      ),
    );
  }
}
