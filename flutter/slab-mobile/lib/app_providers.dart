/// Riverpod wiring: connection config, REST client, catalogs, language
/// preference, and the per-session conversation controller.
///
/// The conversation state machine itself is framework-free
/// (`ConversationController`, external-store pattern — same split as the TS
/// repo) and binds to widgets via `ListenableBuilder`; Riverpod owns
/// cross-screen singletons here.
library;

import 'dart:ui' show PlatformDispatcher;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'package:tdesign_flutter/tdesign_flutter.dart' show TDThemeData;

import 'conversation/conversation_controller.dart';
import 'data/connection_config.dart';
import 'data/rest_client.dart';
import 'l10n/catalog.dart';

/// Both catalogs, loaded in `main()` before `runApp` and injected via
/// provider scope overrides.
class Catalogs {
  const Catalogs({required this.en, required this.zh});
  final SlabCatalog en;
  final SlabCatalog zh;
}

final catalogsProvider = Provider<Catalogs>((ref) => throw UnimplementedError('overridden in main'));

/// The TDesign theme (light + dark in one TDThemeData), loaded from the
/// generated asset in `main()` and injected like the catalogs.
final slabTdThemeProvider = Provider<TDThemeData>(
  (ref) => throw UnimplementedError('overridden in main'),
);

/// Language preference: `auto | en-US | zh-CN` (mirrors the web storage key
/// `slab.ui.language`; stored under the same name in shared_preferences).
final languagePrefProvider = NotifierProvider<LanguagePrefNotifier, String>(LanguagePrefNotifier.new);

class LanguagePrefNotifier extends Notifier<String> {
  static const _key = 'slab.ui.language';

  @override
  String build() => 'auto';

  Future<void> load() async {
    final prefs = await SharedPreferences.getInstance();
    final value = prefs.getString(_key);
    state = value == 'en-US' || value == 'zh-CN' ? value! : 'auto';
  }

  Future<void> set(String preference) async {
    state = preference;
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_key, preference);
  }
}

/// Resolved locale for the current preference (auto follows the platform).
final localeProvider = Provider<String>((ref) {
  final pref = ref.watch(languagePrefProvider);
  if (pref == 'en-US' || pref == 'zh-CN') return pref;
  return SlabCatalog.resolveLocale(PlatformDispatcher.instance.locale.toString());
});

final catalogProvider = Provider<SlabCatalog>((ref) {
  final catalogs = ref.watch(catalogsProvider);
  return ref.watch(localeProvider) == 'zh-CN' ? catalogs.zh : catalogs.en;
});

/// Connection config; `null` until the connect screen completes once.
final connectionConfigProvider = NotifierProvider<ConnectionConfigNotifier, ConnectionConfig?>(ConnectionConfigNotifier.new);

class ConnectionConfigNotifier extends Notifier<ConnectionConfig?> {
  final _store = ConnectionConfigStore();

  @override
  ConnectionConfig? build() => null;

  /// `null` until the connect screen has been completed once (the store
  /// returns null when nothing was saved); the connect page prefills
  /// `kDefaultBaseUrl` itself.
  Future<void> load() async {
    state = await _store.load();
  }

  Future<void> save(ConnectionConfig config) async {
    await _store.save(config);
    state = config;
  }

  Future<void> disconnect() async {
    await _store.clear();
    state = null;
  }
}

final restClientProvider = Provider<SlabRestClient?>((ref) {
  final config = ref.watch(connectionConfigProvider);
  if (config == null) return null;
  final client = SlabRestClient(baseUrl: config.baseUrl, bearerToken: config.bearerToken);
  ref.onDispose(client.dispose);
  return client;
});

/// One conversation controller per slab session (a session change means a new
/// controller — pristine state, mirroring the TS hook keyed on sessionId).
final conversationControllerProvider =
    Provider.family<ConversationController, String>((ref, sessionId) {
  final config = ref.watch(connectionConfigProvider);
  if (config == null) throw StateError('no connection config');
  final controller = ConversationController(sessionId: sessionId, baseUrl: config.baseUrl);
  ref.onDispose(() => controller.dispose());
  controller.start();
  return controller;
});
