/// Bearer-token injection for dio requests.
library;

import 'package:dio/dio.dart';

/// Returns the current bearer token (or null). Kept as a callback — not a
/// stored string — so editing the saved connection config propagates to
/// already-built clients without rebuilding them.
typedef SlabTokenProvider = String? Function();

class SlabAuthInterceptor extends Interceptor {
  SlabAuthInterceptor({required this.tokenProvider});

  final SlabTokenProvider tokenProvider;

  @override
  void onRequest(RequestOptions options, RequestInterceptorHandler handler) {
    final token = tokenProvider();
    if (token != null && token.isNotEmpty) {
      options.headers['Authorization'] = 'Bearer $token';
    }
    handler.next(options);
  }
}
