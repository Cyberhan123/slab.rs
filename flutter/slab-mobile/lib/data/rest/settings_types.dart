/// Settings wire types (`GET /v1/settings`, `PUT /v1/settings/{pmid}`),
/// hand-written tolerant codecs in the repo's data-layer style (see
/// harness_types.dart / model_types.dart). Wire is snake_case;
/// `SettingValue` is an arbitrary decoded-JSON value (`Object?`).
library;

// ── Enums ───────────────────────────────────────────────────────────────────

enum SettingValueType {
  boolean('boolean'),
  integer('integer'),
  unsigned('unsigned'),
  float('float'),
  string('string'),
  array('array'),
  object('object'),
  taggedUnion('tagged_union');

  const SettingValueType(this.wire);
  final String wire;

  static SettingValueType fromWire(String? value) =>
      SettingValueType.values.where((t) => t.wire == value).firstOrNull ?? SettingValueType.string;
}

enum SettingChangeEffect {
  none('none'),
  live('live'),
  needsRestart('needs_restart'),
  needsModelReload('needs_model_reload');

  const SettingChangeEffect(this.wire);
  final String wire;

  static SettingChangeEffect fromWire(String? value) =>
      SettingChangeEffect.values.where((e) => e.wire == value).firstOrNull ?? SettingChangeEffect.none;
}

enum UpdateSettingOperation {
  set('set'),
  unset('unset');

  const UpdateSettingOperation(this.wire);
  final String wire;
}

// ── Schema / property ───────────────────────────────────────────────────────

class SettingPropertySchema {
  const SettingPropertySchema({
    required this.valueType,
    this.enumValues,
    this.minimum,
    this.maximum,
    this.pattern,
    this.jsonSchema,
    this.defaultValue,
    this.secret = false,
    this.multiline = false,
    this.order = 0,
  });

  final SettingValueType valueType;
  final List<String>? enumValues;
  final int? minimum;
  final int? maximum;
  final String? pattern;

  /// Raw JSON-Schema object for structured editors (`array`/`object` types).
  final Object? jsonSchema;
  final Object? defaultValue;
  final bool secret;
  final bool multiline;
  final int order;

  static SettingPropertySchema fromJson(Map<String, Object?> json) => SettingPropertySchema(
        valueType: SettingValueType.fromWire(json['type'] is String ? json['type']! as String : null),
        enumValues:
            json['enum'] is List ? (json['enum']! as List).whereType<String>().toList(growable: false) : null,
        minimum: json['minimum'] is int ? json['minimum']! as int : null,
        maximum: json['maximum'] is int ? json['maximum']! as int : null,
        pattern: json['pattern'] is String ? json['pattern']! as String : null,
        jsonSchema: json['json_schema'],
        defaultValue: json['default_value'],
        secret: json['secret'] == true,
        multiline: json['multiline'] == true,
        order: json['order'] is int ? json['order']! as int : 0,
      );
}

/// Override provenance: an env var or an inherited parent pmid
/// (tagged `{type: "env"|"parent"}`).
sealed class SettingOverrideSource {
  const SettingOverrideSource();
}

class SettingEnvOverride extends SettingOverrideSource {
  const SettingEnvOverride({required this.varName, required this.varValuePresent});
  final String varName;
  final bool varValuePresent;
}

class SettingParentOverride extends SettingOverrideSource {
  const SettingParentOverride({required this.pmid});
  final String pmid;
}

SettingOverrideSource? settingOverrideSourceFromJson(Object? json) {
  if (json is! Map<String, Object?>) return null;
  return switch (json['type']) {
    'env' => SettingEnvOverride(
        varName: json['var_name'] is String ? json['var_name']! as String : '',
        varValuePresent: json['var_value_present'] == true,
      ),
    'parent' => SettingParentOverride(pmid: json['pmid'] is String ? json['pmid']! as String : ''),
    _ => null,
  };
}

class SettingPropertyView {
  const SettingPropertyView({
    required this.pmid,
    required this.label,
    this.descriptionMd = '',
    this.editable = true,
    required this.schema,
    this.effectiveValue,
    this.overrideValue,
    this.isOverridden = false,
    this.changeEffect = SettingChangeEffect.none,
    this.overriddenByJson,
    this.searchTerms = const [],
  });

  final String pmid;
  final String label;
  final String descriptionMd;
  final bool editable;

  final SettingPropertySchema schema;

  /// Current effective value (override ?? default), decoded JSON.
  final Object? effectiveValue;

  /// The user's layer value, when overridden.
  final Object? overrideValue;
  final bool isOverridden;
  final SettingChangeEffect changeEffect;
  final Object? overriddenByJson;
  final List<String> searchTerms;

  /// Decoded override provenance (null when not overridden).
  SettingOverrideSource? get overriddenBy => settingOverrideSourceFromJson(overriddenByJson);

  static SettingPropertyView fromJson(Map<String, Object?> json) => SettingPropertyView(
        pmid: json['pmid'] is String ? json['pmid']! as String : '',
        label: json['label'] is String ? json['label']! as String : '',
        descriptionMd: json['description_md'] is String ? json['description_md']! as String : '',
        editable: json['editable'] != false,
        schema: json['schema'] is Map<String, Object?>
            ? SettingPropertySchema.fromJson(json['schema']! as Map<String, Object?>)
            : const SettingPropertySchema(valueType: SettingValueType.string),
        effectiveValue: json['effective_value'],
        overrideValue: json['override_value'],
        isOverridden: json['is_overridden'] == true,
        changeEffect: SettingChangeEffect.fromWire(
            json['change_effect'] is String ? json['change_effect']! as String : null),
        overriddenByJson: json['overridden_by'],
        searchTerms: json['search_terms'] is List
            ? (json['search_terms']! as List).whereType<String>().toList(growable: false)
            : const [],
      );
}

class SettingsSubsectionView {
  const SettingsSubsectionView({
    required this.id,
    required this.title,
    this.descriptionMd = '',
    this.properties = const [],
  });

  final String id;
  final String title;
  final String descriptionMd;
  final List<SettingPropertyView> properties;

  static SettingsSubsectionView fromJson(Map<String, Object?> json) => SettingsSubsectionView(
        id: json['id'] is String ? json['id']! as String : '',
        title: json['title'] is String ? json['title']! as String : '',
        descriptionMd: json['description_md'] is String ? json['description_md']! as String : '',
        properties: (json['properties'] is List ? json['properties']! as List : const [])
            .whereType<Map<String, Object?>>()
            .map(SettingPropertyView.fromJson)
            .toList(growable: false),
      );
}

class SettingsSectionView {
  const SettingsSectionView({
    required this.id,
    required this.title,
    this.descriptionMd = '',
    this.subsections = const [],
  });

  final String id;
  final String title;
  final String descriptionMd;
  final List<SettingsSubsectionView> subsections;

  static SettingsSectionView fromJson(Map<String, Object?> json) => SettingsSectionView(
        id: json['id'] is String ? json['id']! as String : '',
        title: json['title'] is String ? json['title']! as String : '',
        descriptionMd: json['description_md'] is String ? json['description_md']! as String : '',
        subsections: (json['subsections'] is List ? json['subsections']! as List : const [])
            .whereType<Map<String, Object?>>()
            .map(SettingsSubsectionView.fromJson)
            .toList(growable: false),
      );
}

class SettingsDocumentView {
  const SettingsDocumentView({
    required this.schemaVersion,
    required this.settingsPath,
    this.warnings = const [],
    this.sections = const [],
  });

  final int schemaVersion;
  final String settingsPath;
  final List<String> warnings;
  final List<SettingsSectionView> sections;

  static SettingsDocumentView fromJson(Map<String, Object?> json) => SettingsDocumentView(
        schemaVersion: json['schema_version'] is int ? json['schema_version']! as int : 0,
        settingsPath: json['settings_path'] is String ? json['settings_path']! as String : '',
        warnings:
            json['warnings'] is List ? (json['warnings']! as List).whereType<String>().toList(growable: false) : const [],
        sections: (json['sections'] is List ? json['sections']! as List : const [])
            .whereType<Map<String, Object?>>()
            .map(SettingsSectionView.fromJson)
            .toList(growable: false),
      );
}

/// 400 body from `PUT /v1/settings/{pmid}`: `path` is a JSON-pointer into
/// the submitted value (structured editors map it to the failing sub-field).
class SettingValidationErrorData {
  const SettingValidationErrorData({
    required this.errorType,
    required this.pmid,
    required this.message,
    this.path = '',
  });

  final String errorType;
  final String pmid;
  final String message;
  final String path;

  static SettingValidationErrorData? fromJson(Object? json) {
    if (json is! Map<String, Object?>) return null;
    return SettingValidationErrorData(
      errorType: json['type'] is String ? json['type']! as String : '',
      pmid: json['pmid'] is String ? json['pmid']! as String : '',
      message: json['message'] is String ? json['message']! as String : '',
      path: json['path'] is String ? json['path']! as String : '',
    );
  }
}

/// The typed 400: carries the decoded validation payload for field mapping.
class SettingValidationException implements Exception {
  const SettingValidationException(this.data, this.statusCode);
  final SettingValidationErrorData data;
  final int? statusCode;
  @override
  String toString() => data.message;
}

/// `PUT /v1/settings/{pmid}` body: `set` carries the value, `unset` resets
/// to the default.
Map<String, Object?> updateSettingBody({required bool set, Object? value}) => {
      'op': set ? 'set' : 'unset',
      if (set) 'value': value,
    };
