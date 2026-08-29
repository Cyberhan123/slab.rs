/// Line-level git-diff-style rendering for `apply_patch` / `write_file`
/// change previews. Port of the desktop `patch-diff-view.tsx` classifier:
/// the `*** Begin Patch` dialect (file headers, `@@` chunks, `+`/`-`/context
/// lines), synthesized whole-file previews, unified-diff fallbacks (`+++`/
/// `---`) and heredoc wrappers — anything unrecognized renders plain, so
/// malformed text degrades gracefully.
library;

import 'package:flutter/material.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import 'package:slab_mobile/core/theme/slab_tokens.g.dart';

/// Visual class per classified diff line.
enum DiffLineKind { add, del, context, meta, hunk, plain }

/// Classify one diff line for coloring. See the module doc for the dialects.
DiffLineKind classifyDiffLine(String line) {
  // Patch markers: `*** Begin Patch` / `*** Update File: x` / `*** End of File`…
  if (line.startsWith('***')) return DiffLineKind.meta;
  // Unified-diff file headers (`+++ b/x`, `--- a/x`) — meta, not add/del.
  if (line.startsWith('+++') || line.startsWith('---')) return DiffLineKind.meta;
  if (line.startsWith('@@')) return DiffLineKind.hunk;
  if (line.startsWith('+')) return DiffLineKind.add;
  if (line.startsWith('-')) return DiffLineKind.del;
  if (line.startsWith(' ')) return DiffLineKind.context;
  // Heredoc wrapper lines (`apply_patch <<'EOF'` … `EOF`) are transport, not diff.
  if (line == 'EOF' || line == 'PATCH') return DiffLineKind.meta;
  if (line.startsWith('<<')) return DiffLineKind.meta;
  if (line.startsWith('apply_patch') && line.contains('<<')) return DiffLineKind.meta;
  return DiffLineKind.plain;
}

/// Colored, scroll-bounded diff preview shared by the file-change card and
/// the approval banner.
class PatchDiffView extends StatelessWidget {
  const PatchDiffView({super.key, required this.diff, this.maxHeight = 160});

  final String diff;
  final double maxHeight;

  @override
  Widget build(BuildContext context) {
    final td = context.tTheme;
    final addColor = td.successNormalColor;
    final delColor = td.errorNormalColor;

    return Container(
      constraints: BoxConstraints(maxHeight: maxHeight),
      margin: const EdgeInsets.only(top: 4),
      padding: const EdgeInsets.all(8),
      decoration: BoxDecoration(
        color: td.bgColorSecondaryContainer,
        borderRadius: BorderRadius.circular(SlabMetrics.radiusSm),
      ),
      child: SingleChildScrollView(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            for (final line in diff.split('\n'))
              Text(
                line.isEmpty ? ' ' : line,
                softWrap: false,
                style: TextStyle(
                  fontSize: SlabMetrics.textMicro,
                  height: 1.45,
                  fontFamilyFallback: SlabMetrics.fontMono,
                  // Color per classified line; add/del carry a tinted
                  // background band like the web red/green rows.
                  color: switch (classifyDiffLine(line)) {
                    DiffLineKind.add => addColor,
                    DiffLineKind.del => delColor,
                    DiffLineKind.hunk => td.brandNormalColor,
                    DiffLineKind.context || DiffLineKind.plain => td.textColorPlaceholder,
                    DiffLineKind.meta => td.textColorPrimary,
                  },
                  background: switch (classifyDiffLine(line)) {
                    DiffLineKind.add => Paint()..color = addColor.withValues(alpha: 0.10),
                    DiffLineKind.del => Paint()..color = delColor.withValues(alpha: 0.10),
                    _ => null,
                  },
                ),
              ),
          ],
        ),
      ),
    );
  }
}
