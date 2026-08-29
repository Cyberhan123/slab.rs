/// Factory for the app's dio instances: base options, cross-cutting
/// interceptors (auth, error-envelope mapping) and debug logging.
///
/// Interceptor order matters: auth stamps the outgoing request, the error
/// interceptor maps failures on the way back, and the pretty logger sits last
/// so it observes the final wire shape.
library;

import 'package:dio/dio.dart';
import 'package:flutter/foundation.dart';
import 'package:pretty_dio_logger/pretty_dio_logger.dart';

import 'auth_interceptor.dart';
import 'error_interceptor.dart';

Dio buildSlabDio({
  required Uri baseUrl,
  SlabTokenProvider? tokenProvider,
  bool debugLogging = false,
  List<Interceptor> extraInterceptors = const [],
}) {
  final dio = Dio(
    BaseOptions(
      // Requests are issued with absolute Uris (getUri) so a baseUrl path
      // component cannot silently merge into endpoint paths; this value is a
      // fallback only. No global contentType: dio's default transformer sets
      // `application/json` on Map bodies only, keeping GETs header-identical
      // to the previous package:http client.
      baseUrl: baseUrl.toString(),
      connectTimeout: const Duration(seconds: 8),
      receiveTimeout: const Duration(seconds: 30),
    ),
  );
  if (tokenProvider != null) {
    dio.interceptors.add(SlabAuthInterceptor(tokenProvider: tokenProvider));
  }
  dio.interceptors.addAll(extraInterceptors);
  dio.interceptors.add(SlabErrorInterceptor());
  if (debugLogging && !kReleaseMode) {
    dio.interceptors.add(
      PrettyDioLogger(requestHeader: true, requestBody: true, responseHeader: false, responseBody: true, maxWidth: 120),
    );
  }
  return dio;
}
