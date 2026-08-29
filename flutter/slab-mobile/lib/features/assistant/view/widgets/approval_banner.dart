/// Pending-approval banner: one card per pending request, specialized by
/// kind — command (terminal preview), fileChange (per-change diff previews),
/// plan (plan card body). Choice buttons follow the server-advertised
/// `allowedScopes` (run_once / always_in_workspace / always / deny), falling
/// back to a simple approve/deny pair. Port of `approval-banner.tsx`.
library;

import 'package:flutter/material.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import 'package:slab_mobile/data/harness/harness_types.dart' as proto;
import 'package:slab_mobile/core/theme/slab_tokens.g.dart';
import 'package:slab_mobile/core/theme/td_theme.dart';
import 'messages/diff_view.dart';
import 'messages/plan_card.dart';

class ApprovalBanner extends StatelessWidget {
  const ApprovalBanner({
    super.key,
    required this.approvals,
    required this.onResolve,
    required this.t,
    required this.locale,
    this.onResolveWithScope,
  });

  final List<proto.ApprovalRequest> approvals;

  /// Simple approve/deny path (kept for call sites that don't pick scopes).
  final void Function(proto.ApprovalRequest request, bool approved) onResolve;

  /// Scope-aware path; when provided, scope buttons route here.
  final void Function(proto.ApprovalRequest request, bool approved, proto.ApprovalScope scope)? onResolveWithScope;

  /// Shared-catalog translate fn (`pages.assistant.approval.*`).
  final String Function(String key) t;
  final String locale;

  String _kindLabel(proto.ApprovalKind kind) => switch (kind) {
        proto.ApprovalKind.command => t('pages.assistant.approval.command'),
        proto.ApprovalKind.fileChange => t('pages.assistant.approval.fileChange'),
        proto.ApprovalKind.plan => t('pages.assistant.approval.plan'),
      };

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
              child: _ApprovalCard(
                request: request,
                kindLabel: _kindLabel(request.kind),
                t: t,
                onSimple: onResolve,
                onScoped: onResolveWithScope,
              ),
            ),
        ],
      ),
    );
  }
}

class _ApprovalCard extends StatelessWidget {
  const _ApprovalCard({
    required this.request,
    required this.kindLabel,
    required this.t,
    required this.onSimple,
    required this.onScoped,
  });

  final proto.ApprovalRequest request;
  final String kindLabel;
  final String Function(String key) t;
  final void Function(proto.ApprovalRequest, bool) onSimple;
  final void Function(proto.ApprovalRequest, bool, proto.ApprovalScope)? onScoped;

  @override
  Widget build(BuildContext context) {
    final extras = slabExtras(context);
    final scopes = request.allowedScopes;
    final useScopes = onScoped != null && scopes.isNotEmpty;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(
          '${t('pages.assistant.approval.title')} · $kindLabel',
          style: TextStyle(fontSize: SlabMetrics.textCaption, color: extras.brandGold),
        ),
        const SizedBox(height: 4),
        _body(context),
        const SizedBox(height: 8),
        Row(
          mainAxisAlignment: MainAxisAlignment.end,
          children: [
            if (!useScopes) ...[
              TButton(
                size: TButtonSize.small,
                colorScheme: TButtonColorScheme.defaultTheme,
                variant: TButtonVariant.outline,
                onPressed: () => onSimple(request, false),
                child: Text(t('common.actions.cancel')),
              ),
              const SizedBox(width: 8),
              TButton(
                size: TButtonSize.small,
                colorScheme: TButtonColorScheme.primary,
                onPressed: () => onSimple(request, true),
                child: Text(t('pages.assistant.approval.runOnce')),
              ),
            ] else
              for (final scope in scopes) ...[
                _scopeButton(scope),
                const SizedBox(width: 6),
              ],
          ],
        ),
      ],
    );
  }

  Widget _scopeButton(proto.ApprovalScope scope) {
    final isDeny = scope == proto.ApprovalScope.deny;
    return TButton(
      size: TButtonSize.small,
      colorScheme: isDeny ? TButtonColorScheme.danger : TButtonColorScheme.primary,
      variant: isDeny ? TButtonVariant.outline : TButtonVariant.fill,
      onPressed: () => onScoped!(request, !isDeny, scope),
      child: Text(
        switch (scope) {
          proto.ApprovalScope.runOnce => t('pages.assistant.approval.runOnce'),
          proto.ApprovalScope.alwaysInWorkspace => t('pages.assistant.approval.alwaysInWorkspace'),
          proto.ApprovalScope.always => t('pages.assistant.approval.always'),
          proto.ApprovalScope.deny => t('pages.assistant.approval.deny'),
        },
      ),
    );
  }

  Widget _body(BuildContext context) {
    switch (request.kind) {
      case proto.ApprovalKind.command:
        return Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            if (request.cwd?.isNotEmpty ?? false)
              Text(
                request.cwd!,
                style: TextStyle(fontSize: SlabMetrics.textMicro, color: context.tTheme.textColorPlaceholder),
              ),
            Text(
              '\$ ${request.command ?? ''}',
              style: TextStyle(fontSize: SlabMetrics.textCaption, fontFamilyFallback: SlabMetrics.fontMono),
            ),
          ],
        );
      case proto.ApprovalKind.fileChange:
        return Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            for (final change in request.changes) ...[
              Text(
                change.path,
                style: TextStyle(fontSize: SlabMetrics.textCaption, fontFamilyFallback: SlabMetrics.fontMono),
              ),
              if (change.diff?.isNotEmpty ?? false) PatchDiffView(diff: change.diff!),
            ],
          ],
        );
      case proto.ApprovalKind.plan:
        final plan = decodePlan(request.planSnapshot);
        return plan != null
            ? PlanCardBody(plan: plan)
            : Text(
                request.command ?? 'plan',
                style: TextStyle(fontSize: SlabMetrics.textCaption, fontFamilyFallback: SlabMetrics.fontMono),
              );
    }
  }
}
