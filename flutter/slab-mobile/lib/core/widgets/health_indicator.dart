/// Server health dot + label (TDesign success/error theme tokens only).
library;

import 'package:flutter/material.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

class HealthIndicator extends StatelessWidget {
  const HealthIndicator({super.key, required this.online, required this.onlineLabel, required this.offlineLabel});

  final bool online;
  final String onlineLabel;
  final String offlineLabel;

  @override
  Widget build(BuildContext context) {
    final td = context.tTheme;
    final color = online ? td.successNormalColor : td.errorNormalColor;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Container(
          width: 8,
          height: 8,
          decoration: BoxDecoration(shape: BoxShape.circle, color: color),
        ),
        const SizedBox(width: 6),
        TText(
          online ? onlineLabel : offlineLabel,
          style: TextStyle(fontSize: 11, color: color),
        ),
      ],
    );
  }
}
