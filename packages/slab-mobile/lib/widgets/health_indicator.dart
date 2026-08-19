/// Server health dot + label (success / destructive tokens only — no raw colors).
library;

import 'package:flutter/material.dart';

import '../theme/slab_theme.dart';

class HealthIndicator extends StatelessWidget {
  const HealthIndicator({super.key, required this.online, required this.onlineLabel, required this.offlineLabel});

  final bool online;
  final String onlineLabel;
  final String offlineLabel;

  @override
  Widget build(BuildContext context) {
    final extras = slabExtras(context);
    final scheme = Theme.of(context).colorScheme;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Container(
          width: 8,
          height: 8,
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            color: online ? extras.success : scheme.error,
          ),
        ),
        const SizedBox(width: 6),
        Text(
          online ? onlineLabel : offlineLabel,
          style: Theme.of(context).textTheme.bodySmall?.copyWith(
                color: online ? extras.success : scheme.error,
              ),
        ),
      ],
    );
  }
}
