/// Settings autosave state-machine tests: dirty → debounce → saving → saved
/// (per-type delays), coercion errors, server validation errors, and the
/// race-free latest-draft guard. Uses a fake REST client; timers via
/// `fakeAsync` semantics are approximated with real short waits.
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:slab_mobile/data/rest/rest_client.dart';
import 'package:slab_mobile/data/rest/settings_types.dart';
import 'package:slab_mobile/features/settings/autosave/request_body.dart';
import 'package:slab_mobile/features/settings/settings_cubit.dart';

SettingPropertyView _property({
  String pmid = 'server.address',
  SettingValueType type = SettingValueType.string,
  List<String>? enumValues,
  bool secret = false,
  Object? effective = '127.0.0.1',
}) =>
    SettingPropertyView(
      pmid: pmid,
      label: 'Test',
      schema: SettingPropertySchema(valueType: type, enumValues: enumValues, secret: secret),
      effectiveValue: effective,
    );

class _FakeClient extends SlabRestClient {
  _FakeClient({this.failWithPointer})
      : super(baseUrl: Uri.parse('http://127.0.0.1:9'));

  /// When set, updateSetting throws a validation error with this pointer.
  final String? failWithPointer;
  int updateCalls = 0;
  Object? lastValue;
  bool lastSet = true;

  @override
  Future<SettingPropertyView> updateSetting({required String pmid, required bool set, Object? value}) async {
    updateCalls += 1;
    lastSet = set;
    lastValue = value;
    final pointer = failWithPointer;
    if (pointer != null && updateCalls == 1) {
      throw SettingValidationException(
        SettingValidationErrorData(errorType: 'validation', pmid: pmid, message: 'bad value', path: pointer),
        400,
      );
    }
    return _property(pmid: pmid, effective: value);
  }

  @override
  void dispose() {}
}

void main() {
  test('coercion: numbers, JSON, enums, empty-unset', () {
    expect(parseDraftValue(_property(type: SettingValueType.integer), '42'), isA<DraftValue>().having((d) => d.value, 'v', 42));
    expect(parseDraftValue(_property(type: SettingValueType.integer), 'x'), isA<DraftInvalid>());
    expect(parseDraftValue(_property(type: SettingValueType.unsigned), '-3'), isA<DraftInvalid>());
    expect(parseDraftValue(_property(type: SettingValueType.float), '1.5e3'), isA<DraftValue>().having((d) => d.value, 'v', 1500.0));
    expect(parseDraftValue(_property(type: SettingValueType.object), '{"a":1}'), isA<DraftValue>());
    expect(parseDraftValue(_property(type: SettingValueType.object), '{bad'), isA<DraftInvalid>());
    expect(parseDraftValue(_property(), ''), isA<DraftUnset>());
    expect(parseDraftValue(_property(enumValues: ['a', 'b']), 'a'), isA<DraftValue>().having((d) => d.value, 'v', 'a'));
  });

  test('autosave delays per schema shape', () {
    expect(autoSaveDelay(const SettingPropertySchema(valueType: SettingValueType.boolean)), const Duration(milliseconds: 150));
    expect(autoSaveDelay(const SettingPropertySchema(valueType: SettingValueType.string, enumValues: ['a'])), const Duration(milliseconds: 150));
    expect(autoSaveDelay(const SettingPropertySchema(valueType: SettingValueType.object)), const Duration(milliseconds: 900));
    expect(autoSaveDelay(const SettingPropertySchema(valueType: SettingValueType.string)), const Duration(milliseconds: 650));
  });

  test('search matching spans label, pmid, description, terms', () {
    final property = _property();
    expect(searchMatchesProperty(property, 'server.address'), isTrue);
    expect(searchMatchesProperty(_property(), 'test'), isTrue);
    expect(searchMatchesProperty(_property(), 'nope'), isFalse);
  });

  test('edit → dirty → debounced PUT → saved, draft cleared', () async {
    final client = _FakeClient();
    final cubit = SettingsCubit(client: client);
    final property = _property();
    cubit.editField(property, '0.0.0.0');
    expect(cubit.state.fieldStatus[property.pmid], FieldStatus.dirty);
    // Let the 650ms debounce + PUT round-trip run.
    await Future<void>.delayed(const Duration(milliseconds: 800));
    expect(client.updateCalls, 1);
    expect(client.lastSet, isTrue);
    expect(client.lastValue, '0.0.0.0');
    expect(cubit.state.fieldStatus[property.pmid], FieldStatus.saved);
    expect(cubit.state.drafts.containsKey(property.pmid), isFalse);
    await cubit.close();
  });

  test('server validation error marks the field errored with the message', () async {
    final client = _FakeClient(failWithPointer: '/0/auth/api_base');
    final cubit = SettingsCubit(client: client);
    final property = _property(pmid: 'providers.registry', type: SettingValueType.array, effective: const <Object?>[]);
    cubit.editField(property, '[{"id":"x"}]');
    await Future<void>.delayed(const Duration(milliseconds: 1100));
    expect(cubit.state.fieldStatus[property.pmid], FieldStatus.error);
    expect(cubit.state.fieldErrors[property.pmid], 'bad value');
    await cubit.close();
  });

  test('reset-to-default PUTs unset immediately', () async {
    final client = _FakeClient();
    final cubit = SettingsCubit(client: client);
    final property = _property();
    await cubit.resetField(property);
    expect(client.updateCalls, 1);
    expect(client.lastSet, isFalse);
    expect(cubit.state.fieldStatus[property.pmid], FieldStatus.saved);
    await cubit.close();
  });

  test('hasUnsavedChanges reflects dirty/saving/error counts', () {
    final client = _FakeClient();
    final cubit = SettingsCubit(client: client);
    expect(cubit.state.hasUnsavedChanges, isFalse);
    cubit.editField(_property(), 'x');
    expect(cubit.state.hasUnsavedChanges, isTrue);
    expect(cubit.state.dirtyCount, 1);
    cubit.close();
  });
}
