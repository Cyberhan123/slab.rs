/// Pending-approval banner: one card per pending request with approve / deny
/// (scope = the first the server allows; shared catalog strings, TButtons).
library;

import 'package:flutter/material.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import '../../../../proto/harness_types.dart' as proto;
import '../../../../theme/slab_tokens.g.dart';
import '../../../../theme/td_theme.dart';

class ApprovalBanner extends StatelessWidget {
  const ApprovalBanner({
    super.key,
    required this.approvals,
    required this.onResolve,
    required this.t,
    required this.locale,
  });

  final List<proto.ApprovalRequest> approvals;
  final void Function(proto.ApprovalRequest request, bool approved) onResolve;

  /// Shared-catalog translate fn (`pages.assistant.approval.*`).
  final String Function(String key) t;
  final String locale;

  String _kindLabel(proto.ApprovalKind kind) {
    switch (kind) {
      case proto.ApprovalKind.command:
        return t('pages.assistant.approval.command');
      case proto.ApprovalKind.fileChange:
        return t('pages.assistant.approval.fileChange');
      case proto.ApprovalKind.plan:
        return t('pages.assistant.approval.plan');
    }
  }

  String _detail(proto.ApprovalRequest request) {
    switch (request.kind) {
      case proto.ApprovalKind.command:
        return request.command ?? '';
      case proto.ApprovalKind.fileChange:
        return '${request.changes.length} × file change';
      case proto.ApprovalKind.plan:
        return request.command ?? 'plan';
    }
  }

  @override
  Widget build(BuildContext context) {
    final td = context.tTheme;
    final extras = slabExtras(context);
    if (approvals.isEmpty) return const SizedBox.shrink();

    return Container(
      decoration: BoxDecoration(
        color: td.bgColorContainer,
        borderRadius: BorderRadius.circular(SlabMetrics.radiusLg),
        border: Border.all(color: extras.brandGold),
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          for (final request in approvals)
            Padding(
              padding: const EdgeInsets.fromLTRB(12, 10, 12, 10),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  TText(
                    '${t('pages.assistant.approval.title')} · ${_kindLabel(request.kind)}',
                    style: TextStyle(fontSize: SlabMetrics.textCaption, color: extras.brandGold),
                  ),
                  const SizedBox(height: 4),
                  TText(
                    _detail(request),
                    maxLines: 3,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      fontSize: SlabMetrics.textCaption,
                      fontFamilyFallback: SlabMetrics.fontMono,
                    ),
                  ),
                  const SizedBox(height: 8),
                  Row(
                    mainAxisAlignment: MainAxisAlignment.end,
                    children: [
                      TButton(
                        size: TButtonSize.small,
                        colorScheme: TButtonColorScheme.defaultTheme,
                        variant: TButtonVariant.outline,
                        onPressed: () => onResolve(request, false),
                        child: Text(mobileDeny()),
                      ),
                      const SizedBox(width: 8),
                      TButton(
                        size: TButtonSize.small,
                        colorScheme: TButtonColorScheme.primary,
                        onPressed: () => onResolve(request, true),
                        child: Text(t('pages.assistant.approval.runOnce')),
                      ),
                    ],
                  ),
                ],
              ),
            ),
        ],
      ),
    );
  }

  // Mobile-only chrome string kept local to avoid importing the whole mobile
  // strings map at the call site.
  String mobileDeny() {
    const labels = {'en-US': 'Deny', 'zh-CN': '拒绝'};
    return labels[locale] ?? labels['en-US']!;
  }
}
