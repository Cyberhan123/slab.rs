/// Maps dio failures onto [SlabRestException] so every catch site in the app
/// sees the single app-wide error type.
library;

import 'package:dio/dio.dart';

import 'slab_api_error.dart' show SlabRestException, slabApiErrorWithI18n;

class SlabErrorInterceptor extends Interceptor {
  @override
  void onError(DioException err, ErrorInterceptorHandler handler) {
    final mapped = switch (err.type) {
      DioExceptionType.badResponse when err.response != null => () {
          final (message, i18nKey, i18nParams) = slabApiErrorWithI18n(err.response?.data);
          return SlabRestException(message, err.response?.statusCode, i18nKey: i18nKey, i18nParams: i18nParams);
        }(),
      DioExceptionType.connectionTimeout ||
      DioExceptionType.sendTimeout ||
      DioExceptionType.receiveTimeout ||
      DioExceptionType.connectionError ||
      DioExceptionType.badCertificate =>
        const SlabRestException('network error', null),
      _ => null,
    };
    if (mapped == null) {
      handler.next(err);
    } else {
      // Preserve the original exception (type/response) as context while the
      // `error` field carries what rest_client rethrows.
      handler.next(err.copyWith(error: mapped));
    }
  }
}
