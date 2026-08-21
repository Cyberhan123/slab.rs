/// Pending-approval banner: one card per pending request with approve / deny
/// (scope = the first the server allows; shared catalog strings, TDButtons).
library;

import 'package:flutter/material.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import '../proto/harness_types.dart' as proto;
import '../theme/td_theme.dart';
import '../theme/slab_tokens.g.dart';

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
    final td = TDTheme.of(context);
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
                  TDText(
                    '${t('pages.assistant.approval.title')} · ${_kindLabel(request.kind)}',
                    style: TextStyle(fontSize: SlabMetrics.textCaption, color: extras.brandGold),
                  ),
                  const SizedBox(height: 4),
                  TDText(
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
                      TDButton(
                        text: mobileDeny(),
                        size: TDButtonSize.small,
                        theme: TDButtonTheme.defaultTheme,
                        type: TDButtonType.outline,
                        onTap: () => onResolve(request, false),
                      ),
                      const SizedBox(width: 8),
                      TDButton(
                        text: t('pages.assistant.approval.runOnce'),
                        size: TDButtonSize.small,
                        theme: TDButtonTheme.primary,
                        onTap: () => onResolve(request, true),
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
