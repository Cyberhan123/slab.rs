/// Declarative redirect ladder, the analog of the web `SetupGuard` /
/// connect flow: no saved config → `/connect`; everything else lives under
/// `/sessions` (list) and `/chat/:sessionId`.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../app_providers.dart';
import '../pages/chat_page.dart';
import '../pages/connect_page.dart';
import '../pages/sessions_page.dart';
import '../pages/setup_gate_page.dart';

final appRouterProvider = Provider<GoRouter>((ref) => buildAppRouter(ref));

/// Kept as a plain function (instead of only a provider) so tests can build a
/// router against a custom container.
GoRouter buildAppRouter(Ref ref) => GoRouter(
      initialLocation: '/connect',
      redirect: (context, state) {
        final configured = ref.read(connectionConfigProvider) != null;
        if (!configured && state.matchedLocation != '/connect') return '/connect';
        if (configured && state.matchedLocation == '/connect') return '/sessions';
        return null;
      },
      routes: [
        GoRoute(
          path: '/connect',
          builder: (context, state) => const ConnectPage(),
        ),
        GoRoute(
          path: '/sessions',
          builder: (context, state) => const SessionsPage(),
        ),
        GoRoute(
          path: '/setup',
          builder: (context, state) => const SetupGatePage(),
        ),
        GoRoute(
          path: '/chat/:sessionId',
          builder: (context, state) => ChatPage(
            sessionId: state.pathParameters['sessionId']!,
            sessionName: state.uri.queryParameters['name'],
          ),
        ),
      ],
    );
