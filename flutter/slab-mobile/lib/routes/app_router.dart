/// Declarative redirect ladder, the analog of the web `SetupGuard` /
/// connect flow: no saved config → `/connect`. The tabbed home is a
/// `StatefulShellRoute` (sessions + settings branches under the TTabBar);
/// `/chat/:sessionId` stays a top-level route so it covers the tab bar.
library;

import 'package:go_router/go_router.dart';

import 'package:slab_mobile/core/app/connection_cubit.dart';
import 'package:slab_mobile/features/assistant/view/chat_screen.dart';
import 'package:slab_mobile/features/connect/connect_page.dart';
import 'package:slab_mobile/features/sessions/view/sessions_page.dart';
import 'package:slab_mobile/features/settings/view/settings_page.dart';
import 'package:slab_mobile/features/setup/setup_gate_page.dart';
import 'home_shell.dart';

/// Takes the connection cubit as a parameter (instead of reading the service
/// locator) so tests can build a router against a custom cubit.
GoRouter buildAppRouter({required ConnectionCubit connection}) => GoRouter(
      initialLocation: '/connect',
      redirect: (context, state) {
        final configured = connection.state != null;
        if (!configured && state.matchedLocation != '/connect') return '/connect';
        // `/connect?edit=1` (settings gear on the sessions page) reopens the
        // connect screen to edit the saved URL; a plain `/connect` (cold start
        // via initialLocation) still bounces to the sessions list.
        final editing = state.uri.queryParameters['edit'] == '1';
        if (configured && state.matchedLocation == '/connect' && !editing) return '/sessions';
        return null;
      },
      routes: [
        GoRoute(
          path: '/connect',
          builder: (context, state) => const ConnectPage(),
        ),
        GoRoute(
          path: '/setup',
          builder: (context, state) => const SetupGatePage(),
        ),
        StatefulShellRoute.indexedStack(
          builder: (context, state, shell) => HomeShell(shell: shell),
          branches: [
            StatefulShellBranch(
              routes: [
                GoRoute(
                  path: '/sessions',
                  builder: (context, state) => const SessionsPage(),
                ),
              ],
            ),
            StatefulShellBranch(
              routes: [
                GoRoute(
                  path: '/settings',
                  builder: (context, state) => const SettingsPage(),
                ),
              ],
            ),
          ],
        ),
        GoRoute(
          path: '/chat/:sessionId',
          builder: (context, state) => ChatScreen(
            sessionId: state.pathParameters['sessionId']!,
            sessionName: state.uri.queryParameters['name'],
          ),
        ),
      ],
    );
