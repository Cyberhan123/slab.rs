/// tdesign_flutter component chrome strings, routed through the slab
/// catalogs.
///
/// `TResourceDelegate` is deliberately fully abstract ("so users notice new
/// fields"), so every getter is implemented: the interactive chrome (dialogs,
/// refresh, loading) resolves via `mobileT`/the shared catalog; calendar-only
/// internals — unreachable in the current screens — resolve from the small
/// per-locale tables below (zh mirrors the package's own defaults).
library;

import 'package:flutter/widgets.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import '../app/locale_cubit.dart';
import 'mobile_strings.dart';

class SlabResourceDelegate extends TResourceDelegate {
  SlabResourceDelegate(this.localeCubit);

  final LocaleCubit localeCubit;

  /// Updated on every `setResourceBuilder` call (needs the fresh context).
  BuildContext? context;

  void updateContext(BuildContext value) => context = value;

  /// Always the resolved tag ('en-US' | 'zh-CN'), never the 'auto' preference
  /// — chrome must follow platform resolution exactly like the shared catalog.
  String get _locale => localeCubit.resolvedTag;

  String _t(String key) => mobileT(_locale, key);

  String _cal(String en, String zh) => _locale == 'zh-CN' ? zh : en;

  // ── Interactive chrome (mobile_strings / shared catalog) ────────────────────

  @override
  String get cancel => localeCubit.catalog.t('common.actions.cancel');

  @override
  String get confirm => _t('mobile.common.confirm');

  @override
  String get open => _t('mobile.td.open');

  @override
  String get close => _t('mobile.td.close');

  @override
  String get reset => _t('mobile.td.reset');

  @override
  String get other => _t('mobile.td.other');

  @override
  String get picker => _t('mobile.td.picker');

  /// Picker a11y column label (unreachable in current screens; mirrors the
  /// package default, zh-aware like the calendar internals below).
  @override
  String pickerColumn(int colIndex) => _locale == 'zh-CN' ? '第 $colIndex 列' : 'Column $colIndex';

  @override
  String get loading => _t('mobile.td.loading');

  @override
  String get loadingWithPoint => _t('mobile.td.loadingWithPoint');

  @override
  String get knew => _t('mobile.td.knew');

  @override
  String get refreshing => _t('mobile.td.refreshing');

  @override
  String get releaseRefresh => _t('mobile.td.releaseRefresh');

  @override
  String get pullToRefresh => _t('mobile.td.pullToRefresh');

  @override
  String get completeRefresh => _t('mobile.td.completeRefresh');

  @override
  String get back => _t('mobile.td.back');

  @override
  String get top => _t('mobile.td.top');

  @override
  String get emptyData => _t('mobile.td.emptyData');

  @override
  String get notRated => _t('mobile.td.notRated');

  @override
  String get cascadeLabel => _t('mobile.td.cascadeLabel');

  @override
  String get badgeZero => '0';

  // ── Calendar / duration internals (unreachable in current screens) ─────────

  @override
  String get days => _cal('d', '天');

  @override
  String get hours => _cal('h', '时');

  @override
  String get minutes => _cal('m', '分');

  @override
  String get seconds => _cal('s', '秒');

  @override
  String get milliseconds => _cal('ms', '毫秒');

  @override
  String get yearLabel => _cal('Y', '年');

  @override
  String get monthLabel => _cal('M', '月');

  @override
  String get dateLabel => _cal('D', '日');

  @override
  String get weeksLabel => _cal('W', '周');

  @override
  String get sunday => _cal('Sun', '日');

  @override
  String get monday => _cal('Mon', '一');

  @override
  String get tuesday => _cal('Tue', '二');

  @override
  String get wednesday => _cal('Wed', '三');

  @override
  String get thursday => _cal('Thu', '四');

  @override
  String get friday => _cal('Fri', '五');

  @override
  String get saturday => _cal('Sat', '六');

  @override
  String get year => _cal(' Y', ' 年');

  @override
  String get january => _cal('Jan', '1 月');

  @override
  String get february => _cal('Feb', '2 月');

  @override
  String get march => _cal('Mar', '3 月');

  @override
  String get april => _cal('Apr', '4 月');

  @override
  String get may => _cal('May', '5 月');

  @override
  String get june => _cal('Jun', '6 月');

  @override
  String get july => _cal('Jul', '7 月');

  @override
  String get august => _cal('Aug', '8 月');

  @override
  String get september => _cal('Sep', '9 月');

  @override
  String get october => _cal('Oct', '10 月');

  @override
  String get november => _cal('Nov', '11 月');

  @override
  String get december => _cal('Dec', '12 月');

  @override
  String get time => _cal('Time', '时间');

  @override
  String get start => _cal('Start', '开始');

  @override
  String get end => _cal('End', '结束');
}
