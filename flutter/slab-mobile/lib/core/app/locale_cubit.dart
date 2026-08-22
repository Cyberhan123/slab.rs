/// App-wide locale preference: `auto | en-US | zh-CN`, persisted under the
/// web's `slab.ui.language` key (shared_preferences). Exposes the resolved
/// locale and the active catalog so widgets translate with one lookup.
library;

import 'dart:ui' show PlatformDispatcher;

import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../../l10n/catalog.dart';

class LocaleCubit extends Cubit<String> {
  LocaleCubit({required Catalogs catalogs}) : _catalogs = catalogs, super('auto');

  static const _key = 'slab.ui.language';

  final Catalogs _catalogs;

  /// Preference is `auto` until the user picks a language; resolution then
  /// follows the platform locale (port of the web `normalizeLanguage`).
  String get resolved =>
      state == 'en-US' || state == 'zh-CN' ? state : SlabCatalog.resolveLocale(PlatformDispatcher.instance.locale.toString());

  SlabCatalog get catalog => resolved == 'zh-CN' ? _catalogs.zh : _catalogs.en;

  Future<void> load() async {
    final prefs = await SharedPreferences.getInstance();
    final value = prefs.getString(_key);
    emit(value == 'en-US' || value == 'zh-CN' ? value! : 'auto');
  }

  Future<void> set(String preference) async {
    emit(preference);
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_key, preference);
  }
}
