/// Chat bubble: user bubbles right-aligned (`user-bubble` tokens), assistant
/// bubbles left (`ai-bubble` tokens); markdown text, tool cards, images,
/// reasoning and error parts.
library;

import 'package:flutter/material.dart';
import 'package:gpt_markdown/gpt_markdown.dart';

import '../conversation/turn_items.dart';
import '../theme/slab_theme.dart';
import '../theme/slab_tokens.g.dart';
import 'tool_card.dart';

class MessageBubble extends StatelessWidget {
  const MessageBubble({super.key, required this.message, required this.locale});

  final ChatMessage message;
  final String locale;

  @override
  Widget build(BuildContext context) {
    final extras = slabExtras(context);
    final scheme = Theme.of(context).colorScheme;
    final fromUser = message.fromUser;
    final bubble = fromUser ? extras.userBubble : extras.aiBubble;
    final foreground = fromUser ? extras.userBubbleForeground : extras.aiBubbleForeground;

    return Align(
      alignment: fromUser ? Alignment.centerRight : Alignment.centerLeft,
      child: Container(
        margin: const EdgeInsets.symmetric(vertical: 4, horizontal: 8),
        padding: const EdgeInsets.symmetric(vertical: 10, horizontal: 12),
        constraints: const BoxConstraints(maxWidth: 560),
        decoration: BoxDecoration(
          color: bubble,
          borderRadius: BorderRadius.only(
            topLeft: const Radius.circular(SlabMetrics.radiusLg),
            topRight: const Radius.circular(SlabMetrics.radiusLg),
            bottomLeft: Radius.circular(fromUser ? SlabMetrics.radiusLg : SlabMetrics.radiusSm),
            bottomRight: Radius.circular(fromUser ? SlabMetrics.radiusSm : SlabMetrics.radiusLg),
          ),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            for (final (index, part) in message.parts.indexed) _buildPart(context, part, index, foreground, scheme, extras),
          ],
        ),
      ),
    );
  }

  Widget _buildPart(
    BuildContext context,
    UiPart part,
    int index,
    Color foreground,
    ColorScheme scheme,
    SlabExtras extras,
  ) {
    switch (part) {
      case TextUiPart():
        return Padding(
          padding: EdgeInsets.only(top: index == 0 ? 0 : 6),
          child: GptMarkdown(
            part.text + (part.streaming ? ' ▍' : ''),
            style: TextStyle(fontSize: SlabMetrics.textBody, height: 1.5, color: foreground),
          ),
        );
      case ReasoningUiPart():
        return Padding(
          padding: EdgeInsets.only(top: index == 0 ? 0 : 6),
          child: Opacity(
            opacity: 0.75,
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Icon(Icons.psychology, size: 14, color: scheme.onSurfaceVariant),
                const SizedBox(width: 6),
                Expanded(
                  child: Text(
                    part.streaming ? '${part.text} …' : part.text,
                    maxLines: part.streaming ? 4 : 8,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(fontSize: SlabMetrics.textCaption, height: 1.4, color: scheme.onSurfaceVariant),
                  ),
                ),
              ],
            ),
          ),
        );
      case ToolUiPart():
        return ToolCard(part: part, locale: locale);
      case ImageUiPart():
        return Padding(
          padding: EdgeInsets.only(top: index == 0 ? 0 : 6),
          child: ClipRRect(
            borderRadius: BorderRadius.circular(SlabMetrics.radiusSm),
            child: Image.network(part.url, width: 240, fit: BoxFit.contain, errorBuilder: (_, _, _) => const SizedBox.shrink()),
          ),
        );
      case ErrorUiPart():
        return Padding(
          padding: EdgeInsets.only(top: index == 0 ? 0 : 6),
          child: Row(
            children: [
              Icon(Icons.error_outline, size: 14, color: scheme.error),
              const SizedBox(width: 6),
              Expanded(
                child: Text(
                  part.text,
                  style: TextStyle(fontSize: SlabMetrics.textCaption, color: scheme.error),
                ),
              ),
            ],
          ),
        );
    }
  }
}
