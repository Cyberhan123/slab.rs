/// App-wide connection state: the saved [ConnectionConfig] (null until the
/// connect screen completes once) plus the [SlabRestClient] built from it.
///
/// The client is rebuilt whenever the config changes (save/disconnect) so
/// every consumer shares one transport per config generation. Must be fully
/// loaded before `runApp` — the router redirect reads it synchronously.
library;

import 'package:flutter_bloc/flutter_bloc.dart';

import 'package:slab_mobile/data/local/connection_config.dart';
import 'package:slab_mobile/data/rest/rest_client.dart';

class ConnectionCubit extends Cubit<ConnectionConfig?> {
  /// [client] is a test seam: an injected fake replaces the config-built
  /// client and survives `load()`/`save()` swaps (state still tracks the
  /// persisted config so the router redirect behaves normally).
  ConnectionCubit({SlabRestClient? client})
      : _injectedClient = client,
        _client = client,
        super(null);

  final ConnectionConfigStore _store = ConnectionConfigStore();
  final SlabRestClient? _injectedClient;
  SlabRestClient? _client;

  /// The current REST client; null while unconfigured (or when a test
  /// injected none).
  SlabRestClient? get client => _client;

  Future<void> load() async {
    final config = await _store.load();
    _swapClient(config);
    emit(config);
  }

  Future<void> save(ConnectionConfig config) async {
    await _store.save(config);
    _swapClient(config);
    emit(config);
  }

  Future<void> disconnect() async {
    await _store.clear();
    _swapClient(null);
    emit(null);
  }

  void _swapClient(ConnectionConfig? config) {
    if (_injectedClient != null) {
      _client = _injectedClient;
      return;
    }
    _client?.dispose();
    _client = config == null
        ? null
        : SlabRestClient(baseUrl: config.baseUrl, bearerToken: config.bearerToken);
  }

  @override
  Future<void> close() async {
    _client?.dispose();
    _client = null;
    await super.close();
  }
}
