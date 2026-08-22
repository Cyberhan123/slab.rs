
import 'package:flutter_test/flutter_test.dart';
import 'package:slab_mobile/data/harness/harness_client.dart';
import 'package:slab_mobile/data/harness/harness_methods.dart';

import 'fake_slab_socket.dart';

const baseUrl = 'http://127.0.0.1:3000';

void main() {
  tearDown(() {
    FakeSlabSocket.instanceCount = 0;
  });

  test('harnessWebSocketUri builds ws(s)://…?token=', () {
    final ws = harnessWebSocketUri(Uri.parse('http://192.168.1.5:3000'), 'séance 1');
    expect(ws.scheme, 'ws');
    expect(ws.path, '/v1/agents/harness');
    expect(ws.query, 'token=s%C3%A9ance%201');

    final wss = harnessWebSocketUri(Uri.parse('https://slab.example.com/base'), 't');
    expect(wss.scheme, 'wss');
    expect(wss.host, 'slab.example.com');
  });

  test('open() dials, completes the initialize handshake, and reaches ready', () async {
    final socket = FakeSlabSocket();
    final factory = FakeSocketFactory([socket]);
    final client = HarnessClient(baseUrl: Uri.parse(baseUrl), sessionId: 's1', socketFactory: factory.call);

    await client.open();
    expect(client.status, HarnessStatus.ready);
    expect(socket.countRequests(HarnessMethod.initialize), 1);
    expect(socket.requests.first['params'], {
      'clientInfo': {'name': 'slab-mobile', 'version': '1.0'},
    });

    await client.close();
  });

  test('requests are correlated by id and settle with the matching result', () async {
    final socket = FakeSlabSocket(onRequest: (method, params) => {'echo': method});
    final client = HarnessClient(baseUrl: Uri.parse(baseUrl), sessionId: 's1', socketFactory: FakeSocketFactory([socket]).call);

    final result = await client.sendRequest('thread/list');
    expect(result, {'echo': 'thread/list'});
    await client.close();
  });

  test('error responses reject the caller', () async {
    final socket = FakeSlabSocket();
    final client = HarnessClient(baseUrl: Uri.parse(baseUrl), sessionId: 's1', socketFactory: FakeSocketFactory([socket]).call);

    await client.open();
    // Route around the auto-responder: script the reply from the outside.
    final pending = client.sendRequest('thread/resume');
    await Future<void>.delayed(Duration.zero); // let the request hit the wire
    final id = socket.requests.last['id'];
    socket.serverError(id, 'no thread to resume', code: -32002);
    await expectLater(pending, throwsA(isA<HarnessRpcException>()));
    await client.close();
  });

  test('unexpected drop rejects pending requests, reconnects, and re-initializes', () async {
    // socket1 has NO responder: model/list stays pending across the drop.
    final socket1 = FakeSlabSocket();
    final socket2 = FakeSlabSocket(onRequest: (method, params) => {'ok': true});
    final factory = FakeSocketFactory([socket1, socket2]);
    final client = HarnessClient(
      baseUrl: Uri.parse(baseUrl),
      sessionId: 's1',
      socketFactory: factory.call,
      backoffBase: const Duration(milliseconds: 1),
    );
    final statuses = <HarnessStatus>[];
    client.statusStream.listen(statuses.add);

    await client.open();

    // A request in flight when the transport drops must reject, not hang.
    final pending = client.sendRequest('model/list');
    await Future<void>.delayed(Duration.zero); // request is now pending on the socket
    socket1.drop();
    await expectLater(pending, throwsStateError);

    // The client redials + re-initializes on its own.
    await pumpEventQueue();
    await Future<void>.delayed(const Duration(milliseconds: 20));
    expect(client.status, HarnessStatus.ready);
    expect(factory.created.length, 2);
    expect(socket2.countRequests(HarnessMethod.initialize), 1);
    expect(statuses, containsAll([HarnessStatus.reconnecting, HarnessStatus.ready]));

    await client.close();
  });

  test('user-initiated close() stops reconnection', () async {
    final socket1 = FakeSlabSocket();
    final socket2 = FakeSlabSocket();
    final factory = FakeSocketFactory([socket1, socket2]);
    final client = HarnessClient(
      baseUrl: Uri.parse(baseUrl),
      sessionId: 's1',
      socketFactory: factory.call,
      backoffBase: const Duration(milliseconds: 1),
    );

    await client.open();
    await client.close(); // user close, then simulate a late stream-done
    socket1.drop();
    await pumpEventQueue();
    await Future<void>.delayed(const Duration(milliseconds: 20));
    expect(client.status, HarnessStatus.closed);
    expect(factory.created.length, 1); // no redial after user close
  });

  test('notifications reach subscribers with method and params', () async {
    final socket = FakeSlabSocket();
    final client = HarnessClient(baseUrl: Uri.parse(baseUrl), sessionId: 's1', socketFactory: FakeSocketFactory([socket]).call);
    await client.open();

    final received = <String>[];
    final sub = client.notifications.listen((n) => received.add(n.method));
    socket.push(HarnessNotification.itemAgentMessageDelta, {'itemId': 'a', 'delta': 'hi'});
    await pumpEventQueue();
    expect(received, [HarnessNotification.itemAgentMessageDelta]);
    await sub.cancel();
    await client.close();
  });
}
