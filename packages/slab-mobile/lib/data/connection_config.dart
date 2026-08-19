/// Connection configuration persisted with `shared_preferences`.
///
/// Phase 1 is a LAN/remote tool talking to plain HTTP slab-servers, so plain
/// prefs are acceptable; `flutter_secure_storage` is the documented upgrade
/// path once tokens protect anything sensitive (see the package README).
library;

import 'package:shared_preferences/shared_preferences.dart';

/// `--dart-define=SLAB_API_BASE_URL=...` seeds the first-run default
/// (Android emulator: http://10.0.2.2:3000 → host loopback).
const kDefaultBaseUrl = String.fromEnvironment(
  'SLAB_API_BASE_URL',
  defaultValue: 'http://127.0.0.1:3000',
);

class ConnectionConfig {
  const ConnectionConfig({required this.baseUrl, this.bearerToken});
  final Uri baseUrl;
  final String? bearerToken;

  bool get hasToken => bearerToken != null && bearerToken!.isNotEmpty;
}

class ConnectionConfigStore {
  static const _baseUrlKey = 'slab.mobile.connection.baseUrl';
  static const _tokenKey = 'slab.mobile.connection.token';

  /// `null` until the user completes the connect screen once.
  Future<ConnectionConfig?> load() async {
    final prefs = await SharedPreferences.getInstance();
    final raw = prefs.getString(_baseUrlKey);
    if (raw == null || raw.isEmpty) return null;
    final uri = Uri.tryParse(raw);
    if (uri == null || !uri.hasScheme) return null;
    final token = prefs.getString(_tokenKey);
    return ConnectionConfig(baseUrl: uri, bearerToken: token == null || token.isEmpty ? null : token);
  }

  Future<void> save(ConnectionConfig config) async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_baseUrlKey, config.baseUrl.toString());
    if (config.hasToken) {
      await prefs.setString(_tokenKey, config.bearerToken!);
    } else {
      await prefs.remove(_tokenKey);
    }
  }

  Future<void> clear() async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.remove(_baseUrlKey);
    await prefs.remove(_tokenKey);
  }
}
