/// File-change card: intended per-change diff previews (always shown) plus
/// the live committed-file list while the change applies (`fileChange/
/// outputDelta` streams JSON lines `{path, kind}` — kind A/M/D). Port of
/// `message-tool-file-change-part.tsx`.
library;

import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import 'package:slab_mobile/domain/conversation/turn_items.dart';
import 'package:slab_mobile/core/theme/slab_tokens.g.dart';
import 'diff_view.dart';

/// One committed file parsed from a live output JSON line.
class CommittedFile {
  const CommittedFile({required this.path, required this.kind});
  final String path;
  final String kind; // "A" | "M" | "D" (server contract)
}

/// Parse the live committed-file stream (one JSON object per line).
List<CommittedFile> parseCommittedFiles(String liveOutput) {
  final files = <CommittedFile>[];
  for (final line in liveOutput.split('\n')) {
    final trimmed = line.trim();
    if (trimmed.isEmpty) continue;
    try {
      final decoded = jsonDecode(trimmed);
      if (decoded is Map<String, Object?> && decoded['path'] is String) {
        files.add(CommittedFile(path: decoded['path']! as String, kind: decoded['kind'] is String ? decoded['kind']! as String : ''));
      }
    } catch (_) {
      // Partial line mid-delta — skip until it completes.
    }
  }
  return files;
}

class FileChangeCard extends StatelessWidget {
  const FileChangeCard({super.key, required this.part});

  final ToolUiPart part;

  List<Map<String, Object?>> get _intendedChanges {
    final input = part.input;
    if (input is! Map<String, Object?>) return const [];
    final changes = input['changes'];
    if (changes is! List) return const [];
    return changes.whereType<Map<String, Object?>>().toList(growable: false);
  }

  @override
  Widget build(BuildContext context) {
    final td = context.tTheme;
    final intended = _intendedChanges;
    final committed = part.liveOutput.isNotEmpty ? parseCommittedFiles(part.liveOutput) : const <CommittedFile>[];

    return Container(
      margin: const EdgeInsets.symmetric(vertical: 4),
      padding: const EdgeInsets.all(10),
      decoration: BoxDecoration(
        color: td.bgColorSecondaryContainer,
        borderRadius: BorderRadius.circular(SlabMetrics.radiusMd),
        border: Border.all(color: part.failed ? td.errorNormalColor.withValues(alpha: 0.5) : td.componentStrokeColor),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Icon(TIcons.file_paste, size: 14, color: td.brandNormalColor),
              const SizedBox(width: 6),
              Expanded(
                child: Text(
                  '${intended.length} file change(s)',
                  style: TextStyle(
                    fontSize: SlabMetrics.textCaption,
                    fontWeight: FontWeight.w600,
                    color: td.textColorPrimary,
                  ),
                ),
              ),
              if (committed.isNotEmpty)
                Text(
                  '${committed.length} applied',
                  style: TextStyle(fontSize: SlabMetrics.textMicro, color: td.successNormalColor),
                ),
            ],
          ),
          for (final change in intended) ...[
            const SizedBox(height: 6),
            Text(
              change['path'] is String ? change['path']! as String : '',
              style: TextStyle(
                fontSize: SlabMetrics.textCaption,
                fontFamilyFallback: SlabMetrics.fontMono,
                color: td.textColorSecondary,
              ),
            ),
            if (change['diff'] is String && (change['diff']! as String).isNotEmpty)
              PatchDiffView(diff: change['diff']! as String),
          ],
          if (committed.isNotEmpty) ...[
            const SizedBox(height: 6),
            Wrap(
              spacing: 6,
              runSpacing: 4,
              children: [
                for (final file in committed)
                  Text(
                    '${_kindLabel(file.kind)} ${file.path}',
                    style: TextStyle(
                      fontSize: SlabMetrics.textMicro,
                      fontFamilyFallback: SlabMetrics.fontMono,
                      color: _kindColor(context, file.kind),
                    ),
                  ),
              ],
            ),
          ],
        ],
      ),
    );
  }

  String _kindLabel(String kind) => switch (kind) { 'A' => '+', 'D' => '-', _ => '~' };

  Color _kindColor(BuildContext context, String kind) => switch (kind) {
        'A' => context.tTheme.successNormalColor,
        'D' => context.tTheme.errorNormalColor,
        _ => context.tTheme.textColorSecondary,
      };
}
