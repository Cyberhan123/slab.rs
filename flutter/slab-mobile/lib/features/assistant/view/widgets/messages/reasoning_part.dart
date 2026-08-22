/// Collapsible reasoning block: auto-expanded while streaming, auto-collapsed
/// one second after the stream ends, with a client-side duration label
/// ("thinking…", "Thought for a few seconds", "Thought for Ns"). Port of
/// `message-reasoning-part.tsx` semantics.
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import '../../../../../l10n/catalog.dart';
import '../../../../../theme/slab_tokens.g.dart';
import '../../../../../../conversation/turn_items.dart';

class ReasoningPart extends StatefulWidget {
  const ReasoningPart({super.key, required this.part, required this.catalog});

  final ReasoningUiPart part;
  final SlabCatalog catalog;

  @override
  State<ReasoningPart> createState() => _ReasoningPartState();
}

class _ReasoningPartState extends State<ReasoningPart> {
  bool _expanded = true;
  DateTime? _streamStartedAt;
  Timer? _collapseTimer;

  @override
  void initState() {
    super.initState();
    if (widget.part.streaming) {
      _streamStartedAt = DateTime.now();
    } else {
      // Finalized history reasoning starts collapsed.
      _expanded = false;
    }
  }

  @override
  void didUpdateWidget(ReasoningPart oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.part.streaming && !widget.part.streaming) {
      // Stream just ended: auto-collapse after a beat so the user can catch
      // the final thought, then keep the collapsed summary.
      _collapseTimer?.cancel();
      _collapseTimer = Timer(const Duration(seconds: 1), () {
        if (mounted) setState(() => _expanded = false);
      });
    }
  }

  @override
  void dispose() {
    _collapseTimer?.cancel();
    super.dispose();
  }

  String get _label {
    final t = widget.catalog.t;
    if (widget.part.streaming) return t('pages.assistant.thinking.loading');
    final started = _streamStartedAt;
    if (started == null) return t('pages.assistant.thinking.thoughtForAFewSeconds');
    final seconds = DateTime.now().difference(started).inSeconds;
    if (seconds <= 5) return t('pages.assistant.thinking.thoughtForAFewSeconds');
    return t('pages.assistant.thinking.thoughtForSeconds', {'seconds': '$seconds'});
  }

  @override
  Widget build(BuildContext context) {
    final td = context.tTheme;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        InkWell(
          borderRadius: BorderRadius.circular(SlabMetrics.radiusSm),
          onTap: () {
            _collapseTimer?.cancel();
            setState(() => _expanded = !_expanded);
          },
          child: Padding(
            padding: const EdgeInsets.symmetric(vertical: 2),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(
                  widget.part.streaming ? TIcons.lightbulb : TIcons.time,
                  size: 13,
                  color: td.textColorSecondary,
                ),
                const SizedBox(width: 6),
                Text(
                  _label,
                  style: TextStyle(fontSize: SlabMetrics.textCaption, color: td.textColorSecondary),
                ),
                Icon(
                  _expanded ? TIcons.arrow_up : TIcons.arrow_down,
                  size: 12,
                  color: td.textColorPlaceholder,
                ),
              ],
            ),
          ),
        ),
        if (_expanded)
          Padding(
            padding: const EdgeInsets.only(top: 4),
            child: Text(
              widget.part.text,
              style: TextStyle(
                fontSize: SlabMetrics.textCaption,
                height: 1.5,
                color: td.textColorSecondary,
              ),
            ),
          ),
      ],
    );
  }
}
