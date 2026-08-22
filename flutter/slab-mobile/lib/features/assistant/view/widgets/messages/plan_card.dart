/// Plan tool card: summary, per-status counts, ordered steps with status
/// icons, current-step highlight. The card body is shared by the plan
/// approval banner (same decode). Port of `message-tool-plan-part.tsx`.
library;

import 'package:flutter/material.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import 'package:slab_mobile/core/theme/slab_tokens.g.dart';

enum PlanStepStatus { pending, inProgress, completed, blocked }

class PlanStep {
  const PlanStep({required this.step, required this.status, this.dependsOn = const [], this.resultRef});
  final String step;
  final PlanStepStatus status;
  final List<String> dependsOn;
  final String? resultRef;
}

class PlanData {
  const PlanData({
    required this.planId,
    required this.items,
    required this.counts,
    this.summary,
    this.currentStep,
  });

  final String planId;
  final String? summary;
  final List<PlanStep> items;
  final PlanCounts counts;
  final int? currentStep;
}

class PlanCounts {
  const PlanCounts({required this.pending, required this.inProgress, required this.completed, required this.blocked});
  final int pending;
  final int inProgress;
  final int completed;
  final int blocked;
}

/// Tolerant decode of the `plan` tool payload (`Plan` wire type). `null` when
/// the payload is not plan-shaped — callers render a generic tool card then.
PlanData? decodePlan(Object? json) {
  if (json is! Map<String, Object?>) return null;
  final planId = json['plan_id'] is String ? json['plan_id']! as String : '';
  final rawItems = json['items'] is List ? json['items']! as List : const [];
  final items = <PlanStep>[];
  for (final raw in rawItems) {
    if (raw is! Map<String, Object?>) continue;
    final status = switch (raw['status']) {
      'in_progress' => PlanStepStatus.inProgress,
      'completed' => PlanStepStatus.completed,
      'blocked' => PlanStepStatus.blocked,
      _ => PlanStepStatus.pending,
    };
    items.add(PlanStep(
      step: raw['step'] is String ? raw['step']! as String : '',
      status: status,
      dependsOn: (raw['depends_on'] is List ? raw['depends_on']! as List : const []).whereType<String>().toList(growable: false),
      resultRef: raw['result_ref'] is String ? raw['result_ref']! as String : null,
    ));
  }
  if (planId.isEmpty && items.isEmpty) return null;
  Object? counts = json['counts'];
  counts ??= const {};
  final map = counts is Map<String, Object?> ? counts : const <String, Object?>{};
  int count(String key) => map[key] is int ? map[key]! as int : 0;
  return PlanData(
    planId: planId,
    summary: json['summary'] is String ? json['summary']! as String : null,
    items: items,
    counts: PlanCounts(
      pending: count('pending'),
      inProgress: count('in_progress'),
      completed: count('completed'),
      blocked: count('blocked'),
    ),
    currentStep: json['current_step'] is int ? json['current_step']! as int : null,
  );
}

/// The plan card body — also embedded by the plan approval card.
class PlanCardBody extends StatelessWidget {
  const PlanCardBody({super.key, required this.plan});

  final PlanData plan;

  @override
  Widget build(BuildContext context) {
    final td = context.tTheme;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        if (plan.summary?.isNotEmpty ?? false) ...[
          Text(plan.summary!, style: TextStyle(fontSize: SlabMetrics.textCaption, height: 1.4)),
          const SizedBox(height: 6),
        ],
        Text(
          '${plan.counts.completed}✓ · ${plan.counts.inProgress}▶ · ${plan.counts.pending}○ · ${plan.counts.blocked}⛔',
          style: TextStyle(fontSize: SlabMetrics.textMicro, color: td.textColorSecondary),
        ),
        const SizedBox(height: 6),
        for (final (index, step) in plan.items.indexed)
          _stepRow(context, index, step),
      ],
    );
  }

  Widget _stepRow(BuildContext context, int index, PlanStep step) {
    final td = context.tTheme;
    final isCurrent = plan.currentStep == index;
    final (icon, color) = switch (step.status) {
      PlanStepStatus.completed => (TIcons.check_circle, td.successNormalColor),
      PlanStepStatus.inProgress => (TIcons.loading, td.brandNormalColor),
      PlanStepStatus.blocked => (TIcons.error_circle, td.errorNormalColor),
      PlanStepStatus.pending => (TIcons.circle, td.textColorPlaceholder),
    };
    return Container(
      margin: const EdgeInsets.symmetric(vertical: 2),
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
      decoration: isCurrent
          ? BoxDecoration(
              color: td.brandNormalColor.withValues(alpha: 0.08),
              borderRadius: BorderRadius.circular(SlabMetrics.radiusSm),
            )
          : null,
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(icon, size: 14, color: color),
          const SizedBox(width: 6),
          Expanded(
            child: Text(
              step.step,
              style: TextStyle(
                fontSize: SlabMetrics.textCaption,
                height: 1.4,
                color: isCurrent ? td.textColorPrimary : td.textColorSecondary,
                fontWeight: isCurrent ? FontWeight.w600 : FontWeight.w400,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

/// Full in-stream plan card wrapper.
class PlanCard extends StatelessWidget {
  const PlanCard({super.key, required this.plan});

  final PlanData plan;

  @override
  Widget build(BuildContext context) {
    final td = context.tTheme;
    return Container(
      margin: const EdgeInsets.symmetric(vertical: 4),
      padding: const EdgeInsets.all(10),
      decoration: BoxDecoration(
        color: td.bgColorSecondaryContainer,
        borderRadius: BorderRadius.circular(SlabMetrics.radiusMd),
        border: Border.all(color: td.componentStrokeColor),
      ),
      child: PlanCardBody(plan: plan),
    );
  }
}
