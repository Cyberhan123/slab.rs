import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:slab_mobile/data/harness/json_rpc.dart';

void main() {
  group('classifyInbound', () {
    test('response frame settles by id', () {
      final frame = classifyInbound(jsonDecode('{"jsonrpc":"2.0","id":7,"result":{"ok":true}}'));
      expect(frame, isA<ResponseFrame>());
      expect((frame as ResponseFrame).id, 7);
      expect(frame.result, {'ok': true});
    });

    test('error frame carries code and message', () {
      final frame = classifyInbound(jsonDecode('{"jsonrpc":"2.0","id":2,"error":{"code":-32002,"message":"no thread to resume"}}'));
      expect(frame, isA<ErrorFrame>());
      expect((frame as ErrorFrame).code, -32002);
      expect(frame.message, 'no thread to resume');
    });

    test('notification has method and params, no id', () {
      final frame = classifyInbound(jsonDecode('{"jsonrpc":"2.0","method":"item/agentMessage/delta","params":{"itemId":"a"}}'));
      expect(frame, isA<NotificationFrame>());
      expect((frame as NotificationFrame).method, 'item/agentMessage/delta');
      expect(frame.params, {'itemId': 'a'});
    });

    test('inbound request (method + id) is ignored', () {
      expect(classifyInbound(jsonDecode('{"jsonrpc":"2.0","id":1,"method":"ping"}')), isNull);
    });

    test('invalid jsonrpc version and non-objects are ignored', () {
      expect(classifyInbound(jsonDecode('{"jsonrpc":"1.0","id":1,"result":{}}')), isNull);
      expect(classifyInbound(jsonDecode('[1,2]')), isNull);
      expect(classifyInbound(null), isNull);
      expect(classifyInbound('string'), isNull);
    });

    test('response without result key is invalid', () {
      expect(classifyInbound(jsonDecode('{"jsonrpc":"2.0","id":1}')), isNull);
    });
  });

  group('request framing', () {
    test('params omitted when null, present otherwise', () {
      final without = buildRequestFrame(1, 'thread/list', null);
      expect(without.containsKey('params'), isFalse);

      final withParams = buildRequestFrame(2, 'turn/start', {'threadId': 't'});
      expect(withParams['params'], {'threadId': 't'});
      expect(withParams['jsonrpc'], '2.0');
      // Round-trips through the encoder without loss.
      expect(jsonDecode(encodeFrame(withParams)), withParams);
    });

    test('ids are monotonic', () {
      expect(nextRequestId() + 1, nextRequestId());
    });
  });

  group('parseFrame', () {
    test('null on invalid json, decoded value otherwise', () {
      expect(parseFrame('not json'), isNull);
      expect(parseFrame(null), isNull);
      expect(parseFrame('{"a":1}'), {'a': 1});
    });
  });
}
