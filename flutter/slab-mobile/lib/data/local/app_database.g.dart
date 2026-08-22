// GENERATED CODE - DO NOT MODIFY BY HAND

part of 'app_database.dart';

// ignore_for_file: type=lint
class $AppKvTable extends AppKv with TableInfo<$AppKvTable, AppKvData> {
  @override
  final GeneratedDatabase attachedDatabase;
  final String? _alias;
  $AppKvTable(this.attachedDatabase, [this._alias]);
  static const VerificationMeta _keyMeta = const VerificationMeta('key');
  @override
  late final GeneratedColumn<String> key = GeneratedColumn<String>(
    'key',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _valueMeta = const VerificationMeta('value');
  @override
  late final GeneratedColumn<String> value = GeneratedColumn<String>(
    'value',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  @override
  List<GeneratedColumn> get $columns => [key, value];
  @override
  String get aliasedName => _alias ?? actualTableName;
  @override
  String get actualTableName => $name;
  static const String $name = 'app_kv';
  @override
  VerificationContext validateIntegrity(
    Insertable<AppKvData> instance, {
    bool isInserting = false,
  }) {
    final context = VerificationContext();
    final data = instance.toColumns(true);
    if (data.containsKey('key')) {
      context.handle(
        _keyMeta,
        key.isAcceptableOrUnknown(data['key']!, _keyMeta),
      );
    } else if (isInserting) {
      context.missing(_keyMeta);
    }
    if (data.containsKey('value')) {
      context.handle(
        _valueMeta,
        value.isAcceptableOrUnknown(data['value']!, _valueMeta),
      );
    } else if (isInserting) {
      context.missing(_valueMeta);
    }
    return context;
  }

  @override
  Set<GeneratedColumn> get $primaryKey => {key};
  @override
  AppKvData map(Map<String, dynamic> data, {String? tablePrefix}) {
    final effectivePrefix = tablePrefix != null ? '$tablePrefix.' : '';
    return AppKvData(
      key: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}key'],
      )!,
      value: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}value'],
      )!,
    );
  }

  @override
  $AppKvTable createAlias(String alias) {
    return $AppKvTable(attachedDatabase, alias);
  }
}

class AppKvData extends DataClass implements Insertable<AppKvData> {
  final String key;
  final String value;
  const AppKvData({required this.key, required this.value});
  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    map['key'] = Variable<String>(key);
    map['value'] = Variable<String>(value);
    return map;
  }

  AppKvCompanion toCompanion(bool nullToAbsent) {
    return AppKvCompanion(key: Value(key), value: Value(value));
  }

  factory AppKvData.fromJson(
    Map<String, dynamic> json, {
    ValueSerializer? serializer,
  }) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return AppKvData(
      key: serializer.fromJson<String>(json['key']),
      value: serializer.fromJson<String>(json['value']),
    );
  }
  @override
  Map<String, dynamic> toJson({ValueSerializer? serializer}) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return <String, dynamic>{
      'key': serializer.toJson<String>(key),
      'value': serializer.toJson<String>(value),
    };
  }

  AppKvData copyWith({String? key, String? value}) =>
      AppKvData(key: key ?? this.key, value: value ?? this.value);
  AppKvData copyWithCompanion(AppKvCompanion data) {
    return AppKvData(
      key: data.key.present ? data.key.value : this.key,
      value: data.value.present ? data.value.value : this.value,
    );
  }

  @override
  String toString() {
    return (StringBuffer('AppKvData(')
          ..write('key: $key, ')
          ..write('value: $value')
          ..write(')'))
        .toString();
  }

  @override
  int get hashCode => Object.hash(key, value);
  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is AppKvData &&
          other.key == this.key &&
          other.value == this.value);
}

class AppKvCompanion extends UpdateCompanion<AppKvData> {
  final Value<String> key;
  final Value<String> value;
  final Value<int> rowid;
  const AppKvCompanion({
    this.key = const Value.absent(),
    this.value = const Value.absent(),
    this.rowid = const Value.absent(),
  });
  AppKvCompanion.insert({
    required String key,
    required String value,
    this.rowid = const Value.absent(),
  }) : key = Value(key),
       value = Value(value);
  static Insertable<AppKvData> custom({
    Expression<String>? key,
    Expression<String>? value,
    Expression<int>? rowid,
  }) {
    return RawValuesInsertable({
      if (key != null) 'key': key,
      if (value != null) 'value': value,
      if (rowid != null) 'rowid': rowid,
    });
  }

  AppKvCompanion copyWith({
    Value<String>? key,
    Value<String>? value,
    Value<int>? rowid,
  }) {
    return AppKvCompanion(
      key: key ?? this.key,
      value: value ?? this.value,
      rowid: rowid ?? this.rowid,
    );
  }

  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    if (key.present) {
      map['key'] = Variable<String>(key.value);
    }
    if (value.present) {
      map['value'] = Variable<String>(value.value);
    }
    if (rowid.present) {
      map['rowid'] = Variable<int>(rowid.value);
    }
    return map;
  }

  @override
  String toString() {
    return (StringBuffer('AppKvCompanion(')
          ..write('key: $key, ')
          ..write('value: $value, ')
          ..write('rowid: $rowid')
          ..write(')'))
        .toString();
  }
}

class $SessionLabelsTable extends SessionLabels
    with TableInfo<$SessionLabelsTable, SessionLabel> {
  @override
  final GeneratedDatabase attachedDatabase;
  final String? _alias;
  $SessionLabelsTable(this.attachedDatabase, [this._alias]);
  static const VerificationMeta _sessionIdMeta = const VerificationMeta(
    'sessionId',
  );
  @override
  late final GeneratedColumn<String> sessionId = GeneratedColumn<String>(
    'session_id',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _labelMeta = const VerificationMeta('label');
  @override
  late final GeneratedColumn<String> label = GeneratedColumn<String>(
    'label',
    aliasedName,
    true,
    type: DriftSqlType.string,
    requiredDuringInsert: false,
  );
  static const VerificationMeta _updatedAtMeta = const VerificationMeta(
    'updatedAt',
  );
  @override
  late final GeneratedColumn<DateTime> updatedAt = GeneratedColumn<DateTime>(
    'updated_at',
    aliasedName,
    false,
    type: DriftSqlType.dateTime,
    requiredDuringInsert: true,
  );
  @override
  List<GeneratedColumn> get $columns => [sessionId, label, updatedAt];
  @override
  String get aliasedName => _alias ?? actualTableName;
  @override
  String get actualTableName => $name;
  static const String $name = 'session_labels';
  @override
  VerificationContext validateIntegrity(
    Insertable<SessionLabel> instance, {
    bool isInserting = false,
  }) {
    final context = VerificationContext();
    final data = instance.toColumns(true);
    if (data.containsKey('session_id')) {
      context.handle(
        _sessionIdMeta,
        sessionId.isAcceptableOrUnknown(data['session_id']!, _sessionIdMeta),
      );
    } else if (isInserting) {
      context.missing(_sessionIdMeta);
    }
    if (data.containsKey('label')) {
      context.handle(
        _labelMeta,
        label.isAcceptableOrUnknown(data['label']!, _labelMeta),
      );
    }
    if (data.containsKey('updated_at')) {
      context.handle(
        _updatedAtMeta,
        updatedAt.isAcceptableOrUnknown(data['updated_at']!, _updatedAtMeta),
      );
    } else if (isInserting) {
      context.missing(_updatedAtMeta);
    }
    return context;
  }

  @override
  Set<GeneratedColumn> get $primaryKey => {sessionId};
  @override
  SessionLabel map(Map<String, dynamic> data, {String? tablePrefix}) {
    final effectivePrefix = tablePrefix != null ? '$tablePrefix.' : '';
    return SessionLabel(
      sessionId: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}session_id'],
      )!,
      label: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}label'],
      ),
      updatedAt: attachedDatabase.typeMapping.read(
        DriftSqlType.dateTime,
        data['${effectivePrefix}updated_at'],
      )!,
    );
  }

  @override
  $SessionLabelsTable createAlias(String alias) {
    return $SessionLabelsTable(attachedDatabase, alias);
  }
}

class SessionLabel extends DataClass implements Insertable<SessionLabel> {
  final String sessionId;
  final String? label;
  final DateTime updatedAt;
  const SessionLabel({
    required this.sessionId,
    this.label,
    required this.updatedAt,
  });
  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    map['session_id'] = Variable<String>(sessionId);
    if (!nullToAbsent || label != null) {
      map['label'] = Variable<String>(label);
    }
    map['updated_at'] = Variable<DateTime>(updatedAt);
    return map;
  }

  SessionLabelsCompanion toCompanion(bool nullToAbsent) {
    return SessionLabelsCompanion(
      sessionId: Value(sessionId),
      label: label == null && nullToAbsent
          ? const Value.absent()
          : Value(label),
      updatedAt: Value(updatedAt),
    );
  }

  factory SessionLabel.fromJson(
    Map<String, dynamic> json, {
    ValueSerializer? serializer,
  }) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return SessionLabel(
      sessionId: serializer.fromJson<String>(json['sessionId']),
      label: serializer.fromJson<String?>(json['label']),
      updatedAt: serializer.fromJson<DateTime>(json['updatedAt']),
    );
  }
  @override
  Map<String, dynamic> toJson({ValueSerializer? serializer}) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return <String, dynamic>{
      'sessionId': serializer.toJson<String>(sessionId),
      'label': serializer.toJson<String?>(label),
      'updatedAt': serializer.toJson<DateTime>(updatedAt),
    };
  }

  SessionLabel copyWith({
    String? sessionId,
    Value<String?> label = const Value.absent(),
    DateTime? updatedAt,
  }) => SessionLabel(
    sessionId: sessionId ?? this.sessionId,
    label: label.present ? label.value : this.label,
    updatedAt: updatedAt ?? this.updatedAt,
  );
  SessionLabel copyWithCompanion(SessionLabelsCompanion data) {
    return SessionLabel(
      sessionId: data.sessionId.present ? data.sessionId.value : this.sessionId,
      label: data.label.present ? data.label.value : this.label,
      updatedAt: data.updatedAt.present ? data.updatedAt.value : this.updatedAt,
    );
  }

  @override
  String toString() {
    return (StringBuffer('SessionLabel(')
          ..write('sessionId: $sessionId, ')
          ..write('label: $label, ')
          ..write('updatedAt: $updatedAt')
          ..write(')'))
        .toString();
  }

  @override
  int get hashCode => Object.hash(sessionId, label, updatedAt);
  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is SessionLabel &&
          other.sessionId == this.sessionId &&
          other.label == this.label &&
          other.updatedAt == this.updatedAt);
}

class SessionLabelsCompanion extends UpdateCompanion<SessionLabel> {
  final Value<String> sessionId;
  final Value<String?> label;
  final Value<DateTime> updatedAt;
  final Value<int> rowid;
  const SessionLabelsCompanion({
    this.sessionId = const Value.absent(),
    this.label = const Value.absent(),
    this.updatedAt = const Value.absent(),
    this.rowid = const Value.absent(),
  });
  SessionLabelsCompanion.insert({
    required String sessionId,
    this.label = const Value.absent(),
    required DateTime updatedAt,
    this.rowid = const Value.absent(),
  }) : sessionId = Value(sessionId),
       updatedAt = Value(updatedAt);
  static Insertable<SessionLabel> custom({
    Expression<String>? sessionId,
    Expression<String>? label,
    Expression<DateTime>? updatedAt,
    Expression<int>? rowid,
  }) {
    return RawValuesInsertable({
      if (sessionId != null) 'session_id': sessionId,
      if (label != null) 'label': label,
      if (updatedAt != null) 'updated_at': updatedAt,
      if (rowid != null) 'rowid': rowid,
    });
  }

  SessionLabelsCompanion copyWith({
    Value<String>? sessionId,
    Value<String?>? label,
    Value<DateTime>? updatedAt,
    Value<int>? rowid,
  }) {
    return SessionLabelsCompanion(
      sessionId: sessionId ?? this.sessionId,
      label: label ?? this.label,
      updatedAt: updatedAt ?? this.updatedAt,
      rowid: rowid ?? this.rowid,
    );
  }

  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    if (sessionId.present) {
      map['session_id'] = Variable<String>(sessionId.value);
    }
    if (label.present) {
      map['label'] = Variable<String>(label.value);
    }
    if (updatedAt.present) {
      map['updated_at'] = Variable<DateTime>(updatedAt.value);
    }
    if (rowid.present) {
      map['rowid'] = Variable<int>(rowid.value);
    }
    return map;
  }

  @override
  String toString() {
    return (StringBuffer('SessionLabelsCompanion(')
          ..write('sessionId: $sessionId, ')
          ..write('label: $label, ')
          ..write('updatedAt: $updatedAt, ')
          ..write('rowid: $rowid')
          ..write(')'))
        .toString();
  }
}

class $ComposerDraftsTable extends ComposerDrafts
    with TableInfo<$ComposerDraftsTable, ComposerDraft> {
  @override
  final GeneratedDatabase attachedDatabase;
  final String? _alias;
  $ComposerDraftsTable(this.attachedDatabase, [this._alias]);
  static const VerificationMeta _sessionIdMeta = const VerificationMeta(
    'sessionId',
  );
  @override
  late final GeneratedColumn<String> sessionId = GeneratedColumn<String>(
    'session_id',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _contentMeta = const VerificationMeta(
    'content',
  );
  @override
  late final GeneratedColumn<String> content = GeneratedColumn<String>(
    'content',
    aliasedName,
    false,
    type: DriftSqlType.string,
    requiredDuringInsert: true,
  );
  static const VerificationMeta _planModeMeta = const VerificationMeta(
    'planMode',
  );
  @override
  late final GeneratedColumn<bool> planMode = GeneratedColumn<bool>(
    'plan_mode',
    aliasedName,
    false,
    type: DriftSqlType.bool,
    requiredDuringInsert: false,
    defaultConstraints: GeneratedColumn.constraintIsAlways(
      'CHECK ("plan_mode" IN (0, 1))',
    ),
    defaultValue: const Constant(false),
  );
  static const VerificationMeta _effortMeta = const VerificationMeta('effort');
  @override
  late final GeneratedColumn<String> effort = GeneratedColumn<String>(
    'effort',
    aliasedName,
    true,
    type: DriftSqlType.string,
    requiredDuringInsert: false,
  );
  static const VerificationMeta _permissionModeMeta = const VerificationMeta(
    'permissionMode',
  );
  @override
  late final GeneratedColumn<String> permissionMode = GeneratedColumn<String>(
    'permission_mode',
    aliasedName,
    true,
    type: DriftSqlType.string,
    requiredDuringInsert: false,
  );
  static const VerificationMeta _updatedAtMeta = const VerificationMeta(
    'updatedAt',
  );
  @override
  late final GeneratedColumn<DateTime> updatedAt = GeneratedColumn<DateTime>(
    'updated_at',
    aliasedName,
    false,
    type: DriftSqlType.dateTime,
    requiredDuringInsert: true,
  );
  @override
  List<GeneratedColumn> get $columns => [
    sessionId,
    content,
    planMode,
    effort,
    permissionMode,
    updatedAt,
  ];
  @override
  String get aliasedName => _alias ?? actualTableName;
  @override
  String get actualTableName => $name;
  static const String $name = 'composer_drafts';
  @override
  VerificationContext validateIntegrity(
    Insertable<ComposerDraft> instance, {
    bool isInserting = false,
  }) {
    final context = VerificationContext();
    final data = instance.toColumns(true);
    if (data.containsKey('session_id')) {
      context.handle(
        _sessionIdMeta,
        sessionId.isAcceptableOrUnknown(data['session_id']!, _sessionIdMeta),
      );
    } else if (isInserting) {
      context.missing(_sessionIdMeta);
    }
    if (data.containsKey('content')) {
      context.handle(
        _contentMeta,
        content.isAcceptableOrUnknown(data['content']!, _contentMeta),
      );
    } else if (isInserting) {
      context.missing(_contentMeta);
    }
    if (data.containsKey('plan_mode')) {
      context.handle(
        _planModeMeta,
        planMode.isAcceptableOrUnknown(data['plan_mode']!, _planModeMeta),
      );
    }
    if (data.containsKey('effort')) {
      context.handle(
        _effortMeta,
        effort.isAcceptableOrUnknown(data['effort']!, _effortMeta),
      );
    }
    if (data.containsKey('permission_mode')) {
      context.handle(
        _permissionModeMeta,
        permissionMode.isAcceptableOrUnknown(
          data['permission_mode']!,
          _permissionModeMeta,
        ),
      );
    }
    if (data.containsKey('updated_at')) {
      context.handle(
        _updatedAtMeta,
        updatedAt.isAcceptableOrUnknown(data['updated_at']!, _updatedAtMeta),
      );
    } else if (isInserting) {
      context.missing(_updatedAtMeta);
    }
    return context;
  }

  @override
  Set<GeneratedColumn> get $primaryKey => {sessionId};
  @override
  ComposerDraft map(Map<String, dynamic> data, {String? tablePrefix}) {
    final effectivePrefix = tablePrefix != null ? '$tablePrefix.' : '';
    return ComposerDraft(
      sessionId: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}session_id'],
      )!,
      content: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}content'],
      )!,
      planMode: attachedDatabase.typeMapping.read(
        DriftSqlType.bool,
        data['${effectivePrefix}plan_mode'],
      )!,
      effort: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}effort'],
      ),
      permissionMode: attachedDatabase.typeMapping.read(
        DriftSqlType.string,
        data['${effectivePrefix}permission_mode'],
      ),
      updatedAt: attachedDatabase.typeMapping.read(
        DriftSqlType.dateTime,
        data['${effectivePrefix}updated_at'],
      )!,
    );
  }

  @override
  $ComposerDraftsTable createAlias(String alias) {
    return $ComposerDraftsTable(attachedDatabase, alias);
  }
}

class ComposerDraft extends DataClass implements Insertable<ComposerDraft> {
  final String sessionId;
  final String content;
  final bool planMode;
  final String? effort;
  final String? permissionMode;
  final DateTime updatedAt;
  const ComposerDraft({
    required this.sessionId,
    required this.content,
    required this.planMode,
    this.effort,
    this.permissionMode,
    required this.updatedAt,
  });
  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    map['session_id'] = Variable<String>(sessionId);
    map['content'] = Variable<String>(content);
    map['plan_mode'] = Variable<bool>(planMode);
    if (!nullToAbsent || effort != null) {
      map['effort'] = Variable<String>(effort);
    }
    if (!nullToAbsent || permissionMode != null) {
      map['permission_mode'] = Variable<String>(permissionMode);
    }
    map['updated_at'] = Variable<DateTime>(updatedAt);
    return map;
  }

  ComposerDraftsCompanion toCompanion(bool nullToAbsent) {
    return ComposerDraftsCompanion(
      sessionId: Value(sessionId),
      content: Value(content),
      planMode: Value(planMode),
      effort: effort == null && nullToAbsent
          ? const Value.absent()
          : Value(effort),
      permissionMode: permissionMode == null && nullToAbsent
          ? const Value.absent()
          : Value(permissionMode),
      updatedAt: Value(updatedAt),
    );
  }

  factory ComposerDraft.fromJson(
    Map<String, dynamic> json, {
    ValueSerializer? serializer,
  }) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return ComposerDraft(
      sessionId: serializer.fromJson<String>(json['sessionId']),
      content: serializer.fromJson<String>(json['content']),
      planMode: serializer.fromJson<bool>(json['planMode']),
      effort: serializer.fromJson<String?>(json['effort']),
      permissionMode: serializer.fromJson<String?>(json['permissionMode']),
      updatedAt: serializer.fromJson<DateTime>(json['updatedAt']),
    );
  }
  @override
  Map<String, dynamic> toJson({ValueSerializer? serializer}) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return <String, dynamic>{
      'sessionId': serializer.toJson<String>(sessionId),
      'content': serializer.toJson<String>(content),
      'planMode': serializer.toJson<bool>(planMode),
      'effort': serializer.toJson<String?>(effort),
      'permissionMode': serializer.toJson<String?>(permissionMode),
      'updatedAt': serializer.toJson<DateTime>(updatedAt),
    };
  }

  ComposerDraft copyWith({
    String? sessionId,
    String? content,
    bool? planMode,
    Value<String?> effort = const Value.absent(),
    Value<String?> permissionMode = const Value.absent(),
    DateTime? updatedAt,
  }) => ComposerDraft(
    sessionId: sessionId ?? this.sessionId,
    content: content ?? this.content,
    planMode: planMode ?? this.planMode,
    effort: effort.present ? effort.value : this.effort,
    permissionMode: permissionMode.present
        ? permissionMode.value
        : this.permissionMode,
    updatedAt: updatedAt ?? this.updatedAt,
  );
  ComposerDraft copyWithCompanion(ComposerDraftsCompanion data) {
    return ComposerDraft(
      sessionId: data.sessionId.present ? data.sessionId.value : this.sessionId,
      content: data.content.present ? data.content.value : this.content,
      planMode: data.planMode.present ? data.planMode.value : this.planMode,
      effort: data.effort.present ? data.effort.value : this.effort,
      permissionMode: data.permissionMode.present
          ? data.permissionMode.value
          : this.permissionMode,
      updatedAt: data.updatedAt.present ? data.updatedAt.value : this.updatedAt,
    );
  }

  @override
  String toString() {
    return (StringBuffer('ComposerDraft(')
          ..write('sessionId: $sessionId, ')
          ..write('content: $content, ')
          ..write('planMode: $planMode, ')
          ..write('effort: $effort, ')
          ..write('permissionMode: $permissionMode, ')
          ..write('updatedAt: $updatedAt')
          ..write(')'))
        .toString();
  }

  @override
  int get hashCode => Object.hash(
    sessionId,
    content,
    planMode,
    effort,
    permissionMode,
    updatedAt,
  );
  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is ComposerDraft &&
          other.sessionId == this.sessionId &&
          other.content == this.content &&
          other.planMode == this.planMode &&
          other.effort == this.effort &&
          other.permissionMode == this.permissionMode &&
          other.updatedAt == this.updatedAt);
}

class ComposerDraftsCompanion extends UpdateCompanion<ComposerDraft> {
  final Value<String> sessionId;
  final Value<String> content;
  final Value<bool> planMode;
  final Value<String?> effort;
  final Value<String?> permissionMode;
  final Value<DateTime> updatedAt;
  final Value<int> rowid;
  const ComposerDraftsCompanion({
    this.sessionId = const Value.absent(),
    this.content = const Value.absent(),
    this.planMode = const Value.absent(),
    this.effort = const Value.absent(),
    this.permissionMode = const Value.absent(),
    this.updatedAt = const Value.absent(),
    this.rowid = const Value.absent(),
  });
  ComposerDraftsCompanion.insert({
    required String sessionId,
    required String content,
    this.planMode = const Value.absent(),
    this.effort = const Value.absent(),
    this.permissionMode = const Value.absent(),
    required DateTime updatedAt,
    this.rowid = const Value.absent(),
  }) : sessionId = Value(sessionId),
       content = Value(content),
       updatedAt = Value(updatedAt);
  static Insertable<ComposerDraft> custom({
    Expression<String>? sessionId,
    Expression<String>? content,
    Expression<bool>? planMode,
    Expression<String>? effort,
    Expression<String>? permissionMode,
    Expression<DateTime>? updatedAt,
    Expression<int>? rowid,
  }) {
    return RawValuesInsertable({
      if (sessionId != null) 'session_id': sessionId,
      if (content != null) 'content': content,
      if (planMode != null) 'plan_mode': planMode,
      if (effort != null) 'effort': effort,
      if (permissionMode != null) 'permission_mode': permissionMode,
      if (updatedAt != null) 'updated_at': updatedAt,
      if (rowid != null) 'rowid': rowid,
    });
  }

  ComposerDraftsCompanion copyWith({
    Value<String>? sessionId,
    Value<String>? content,
    Value<bool>? planMode,
    Value<String?>? effort,
    Value<String?>? permissionMode,
    Value<DateTime>? updatedAt,
    Value<int>? rowid,
  }) {
    return ComposerDraftsCompanion(
      sessionId: sessionId ?? this.sessionId,
      content: content ?? this.content,
      planMode: planMode ?? this.planMode,
      effort: effort ?? this.effort,
      permissionMode: permissionMode ?? this.permissionMode,
      updatedAt: updatedAt ?? this.updatedAt,
      rowid: rowid ?? this.rowid,
    );
  }

  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    if (sessionId.present) {
      map['session_id'] = Variable<String>(sessionId.value);
    }
    if (content.present) {
      map['content'] = Variable<String>(content.value);
    }
    if (planMode.present) {
      map['plan_mode'] = Variable<bool>(planMode.value);
    }
    if (effort.present) {
      map['effort'] = Variable<String>(effort.value);
    }
    if (permissionMode.present) {
      map['permission_mode'] = Variable<String>(permissionMode.value);
    }
    if (updatedAt.present) {
      map['updated_at'] = Variable<DateTime>(updatedAt.value);
    }
    if (rowid.present) {
      map['rowid'] = Variable<int>(rowid.value);
    }
    return map;
  }

  @override
  String toString() {
    return (StringBuffer('ComposerDraftsCompanion(')
          ..write('sessionId: $sessionId, ')
          ..write('content: $content, ')
          ..write('planMode: $planMode, ')
          ..write('effort: $effort, ')
          ..write('permissionMode: $permissionMode, ')
          ..write('updatedAt: $updatedAt, ')
          ..write('rowid: $rowid')
          ..write(')'))
        .toString();
  }
}

abstract class _$AppDatabase extends GeneratedDatabase {
  _$AppDatabase(QueryExecutor e) : super(e);
  $AppDatabaseManager get managers => $AppDatabaseManager(this);
  late final $AppKvTable appKv = $AppKvTable(this);
  late final $SessionLabelsTable sessionLabels = $SessionLabelsTable(this);
  late final $ComposerDraftsTable composerDrafts = $ComposerDraftsTable(this);
  @override
  Iterable<TableInfo<Table, Object?>> get allTables =>
      allSchemaEntities.whereType<TableInfo<Table, Object?>>();
  @override
  List<DatabaseSchemaEntity> get allSchemaEntities => [
    appKv,
    sessionLabels,
    composerDrafts,
  ];
}

typedef $$AppKvTableCreateCompanionBuilder =
    AppKvCompanion Function({
      required String key,
      required String value,
      Value<int> rowid,
    });
typedef $$AppKvTableUpdateCompanionBuilder =
    AppKvCompanion Function({
      Value<String> key,
      Value<String> value,
      Value<int> rowid,
    });

class $$AppKvTableFilterComposer extends Composer<_$AppDatabase, $AppKvTable> {
  $$AppKvTableFilterComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnFilters<String> get key => $composableBuilder(
    column: $table.key,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get value => $composableBuilder(
    column: $table.value,
    builder: (column) => ColumnFilters(column),
  );
}

class $$AppKvTableOrderingComposer
    extends Composer<_$AppDatabase, $AppKvTable> {
  $$AppKvTableOrderingComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnOrderings<String> get key => $composableBuilder(
    column: $table.key,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get value => $composableBuilder(
    column: $table.value,
    builder: (column) => ColumnOrderings(column),
  );
}

class $$AppKvTableAnnotationComposer
    extends Composer<_$AppDatabase, $AppKvTable> {
  $$AppKvTableAnnotationComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  GeneratedColumn<String> get key =>
      $composableBuilder(column: $table.key, builder: (column) => column);

  GeneratedColumn<String> get value =>
      $composableBuilder(column: $table.value, builder: (column) => column);
}

class $$AppKvTableTableManager
    extends
        RootTableManager<
          _$AppDatabase,
          $AppKvTable,
          AppKvData,
          $$AppKvTableFilterComposer,
          $$AppKvTableOrderingComposer,
          $$AppKvTableAnnotationComposer,
          $$AppKvTableCreateCompanionBuilder,
          $$AppKvTableUpdateCompanionBuilder,
          (AppKvData, BaseReferences<_$AppDatabase, $AppKvTable, AppKvData>),
          AppKvData,
          PrefetchHooks Function()
        > {
  $$AppKvTableTableManager(_$AppDatabase db, $AppKvTable table)
    : super(
        TableManagerState(
          db: db,
          table: table,
          createFilteringComposer: () =>
              $$AppKvTableFilterComposer($db: db, $table: table),
          createOrderingComposer: () =>
              $$AppKvTableOrderingComposer($db: db, $table: table),
          createComputedFieldComposer: () =>
              $$AppKvTableAnnotationComposer($db: db, $table: table),
          updateCompanionCallback:
              ({
                Value<String> key = const Value.absent(),
                Value<String> value = const Value.absent(),
                Value<int> rowid = const Value.absent(),
              }) => AppKvCompanion(key: key, value: value, rowid: rowid),
          createCompanionCallback:
              ({
                required String key,
                required String value,
                Value<int> rowid = const Value.absent(),
              }) => AppKvCompanion.insert(key: key, value: value, rowid: rowid),
          withReferenceMapper: (p0) => p0
              .map((e) => (e.readTable(table), BaseReferences(db, table, e)))
              .toList(),
          prefetchHooksCallback: null,
        ),
      );
}

typedef $$AppKvTableProcessedTableManager =
    ProcessedTableManager<
      _$AppDatabase,
      $AppKvTable,
      AppKvData,
      $$AppKvTableFilterComposer,
      $$AppKvTableOrderingComposer,
      $$AppKvTableAnnotationComposer,
      $$AppKvTableCreateCompanionBuilder,
      $$AppKvTableUpdateCompanionBuilder,
      (AppKvData, BaseReferences<_$AppDatabase, $AppKvTable, AppKvData>),
      AppKvData,
      PrefetchHooks Function()
    >;
typedef $$SessionLabelsTableCreateCompanionBuilder =
    SessionLabelsCompanion Function({
      required String sessionId,
      Value<String?> label,
      required DateTime updatedAt,
      Value<int> rowid,
    });
typedef $$SessionLabelsTableUpdateCompanionBuilder =
    SessionLabelsCompanion Function({
      Value<String> sessionId,
      Value<String?> label,
      Value<DateTime> updatedAt,
      Value<int> rowid,
    });

class $$SessionLabelsTableFilterComposer
    extends Composer<_$AppDatabase, $SessionLabelsTable> {
  $$SessionLabelsTableFilterComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnFilters<String> get sessionId => $composableBuilder(
    column: $table.sessionId,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get label => $composableBuilder(
    column: $table.label,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<DateTime> get updatedAt => $composableBuilder(
    column: $table.updatedAt,
    builder: (column) => ColumnFilters(column),
  );
}

class $$SessionLabelsTableOrderingComposer
    extends Composer<_$AppDatabase, $SessionLabelsTable> {
  $$SessionLabelsTableOrderingComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnOrderings<String> get sessionId => $composableBuilder(
    column: $table.sessionId,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get label => $composableBuilder(
    column: $table.label,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<DateTime> get updatedAt => $composableBuilder(
    column: $table.updatedAt,
    builder: (column) => ColumnOrderings(column),
  );
}

class $$SessionLabelsTableAnnotationComposer
    extends Composer<_$AppDatabase, $SessionLabelsTable> {
  $$SessionLabelsTableAnnotationComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  GeneratedColumn<String> get sessionId =>
      $composableBuilder(column: $table.sessionId, builder: (column) => column);

  GeneratedColumn<String> get label =>
      $composableBuilder(column: $table.label, builder: (column) => column);

  GeneratedColumn<DateTime> get updatedAt =>
      $composableBuilder(column: $table.updatedAt, builder: (column) => column);
}

class $$SessionLabelsTableTableManager
    extends
        RootTableManager<
          _$AppDatabase,
          $SessionLabelsTable,
          SessionLabel,
          $$SessionLabelsTableFilterComposer,
          $$SessionLabelsTableOrderingComposer,
          $$SessionLabelsTableAnnotationComposer,
          $$SessionLabelsTableCreateCompanionBuilder,
          $$SessionLabelsTableUpdateCompanionBuilder,
          (
            SessionLabel,
            BaseReferences<_$AppDatabase, $SessionLabelsTable, SessionLabel>,
          ),
          SessionLabel,
          PrefetchHooks Function()
        > {
  $$SessionLabelsTableTableManager(_$AppDatabase db, $SessionLabelsTable table)
    : super(
        TableManagerState(
          db: db,
          table: table,
          createFilteringComposer: () =>
              $$SessionLabelsTableFilterComposer($db: db, $table: table),
          createOrderingComposer: () =>
              $$SessionLabelsTableOrderingComposer($db: db, $table: table),
          createComputedFieldComposer: () =>
              $$SessionLabelsTableAnnotationComposer($db: db, $table: table),
          updateCompanionCallback:
              ({
                Value<String> sessionId = const Value.absent(),
                Value<String?> label = const Value.absent(),
                Value<DateTime> updatedAt = const Value.absent(),
                Value<int> rowid = const Value.absent(),
              }) => SessionLabelsCompanion(
                sessionId: sessionId,
                label: label,
                updatedAt: updatedAt,
                rowid: rowid,
              ),
          createCompanionCallback:
              ({
                required String sessionId,
                Value<String?> label = const Value.absent(),
                required DateTime updatedAt,
                Value<int> rowid = const Value.absent(),
              }) => SessionLabelsCompanion.insert(
                sessionId: sessionId,
                label: label,
                updatedAt: updatedAt,
                rowid: rowid,
              ),
          withReferenceMapper: (p0) => p0
              .map((e) => (e.readTable(table), BaseReferences(db, table, e)))
              .toList(),
          prefetchHooksCallback: null,
        ),
      );
}

typedef $$SessionLabelsTableProcessedTableManager =
    ProcessedTableManager<
      _$AppDatabase,
      $SessionLabelsTable,
      SessionLabel,
      $$SessionLabelsTableFilterComposer,
      $$SessionLabelsTableOrderingComposer,
      $$SessionLabelsTableAnnotationComposer,
      $$SessionLabelsTableCreateCompanionBuilder,
      $$SessionLabelsTableUpdateCompanionBuilder,
      (
        SessionLabel,
        BaseReferences<_$AppDatabase, $SessionLabelsTable, SessionLabel>,
      ),
      SessionLabel,
      PrefetchHooks Function()
    >;
typedef $$ComposerDraftsTableCreateCompanionBuilder =
    ComposerDraftsCompanion Function({
      required String sessionId,
      required String content,
      Value<bool> planMode,
      Value<String?> effort,
      Value<String?> permissionMode,
      required DateTime updatedAt,
      Value<int> rowid,
    });
typedef $$ComposerDraftsTableUpdateCompanionBuilder =
    ComposerDraftsCompanion Function({
      Value<String> sessionId,
      Value<String> content,
      Value<bool> planMode,
      Value<String?> effort,
      Value<String?> permissionMode,
      Value<DateTime> updatedAt,
      Value<int> rowid,
    });

class $$ComposerDraftsTableFilterComposer
    extends Composer<_$AppDatabase, $ComposerDraftsTable> {
  $$ComposerDraftsTableFilterComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnFilters<String> get sessionId => $composableBuilder(
    column: $table.sessionId,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get content => $composableBuilder(
    column: $table.content,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<bool> get planMode => $composableBuilder(
    column: $table.planMode,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get effort => $composableBuilder(
    column: $table.effort,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<String> get permissionMode => $composableBuilder(
    column: $table.permissionMode,
    builder: (column) => ColumnFilters(column),
  );

  ColumnFilters<DateTime> get updatedAt => $composableBuilder(
    column: $table.updatedAt,
    builder: (column) => ColumnFilters(column),
  );
}

class $$ComposerDraftsTableOrderingComposer
    extends Composer<_$AppDatabase, $ComposerDraftsTable> {
  $$ComposerDraftsTableOrderingComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnOrderings<String> get sessionId => $composableBuilder(
    column: $table.sessionId,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get content => $composableBuilder(
    column: $table.content,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<bool> get planMode => $composableBuilder(
    column: $table.planMode,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get effort => $composableBuilder(
    column: $table.effort,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<String> get permissionMode => $composableBuilder(
    column: $table.permissionMode,
    builder: (column) => ColumnOrderings(column),
  );

  ColumnOrderings<DateTime> get updatedAt => $composableBuilder(
    column: $table.updatedAt,
    builder: (column) => ColumnOrderings(column),
  );
}

class $$ComposerDraftsTableAnnotationComposer
    extends Composer<_$AppDatabase, $ComposerDraftsTable> {
  $$ComposerDraftsTableAnnotationComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  GeneratedColumn<String> get sessionId =>
      $composableBuilder(column: $table.sessionId, builder: (column) => column);

  GeneratedColumn<String> get content =>
      $composableBuilder(column: $table.content, builder: (column) => column);

  GeneratedColumn<bool> get planMode =>
      $composableBuilder(column: $table.planMode, builder: (column) => column);

  GeneratedColumn<String> get effort =>
      $composableBuilder(column: $table.effort, builder: (column) => column);

  GeneratedColumn<String> get permissionMode => $composableBuilder(
    column: $table.permissionMode,
    builder: (column) => column,
  );

  GeneratedColumn<DateTime> get updatedAt =>
      $composableBuilder(column: $table.updatedAt, builder: (column) => column);
}

class $$ComposerDraftsTableTableManager
    extends
        RootTableManager<
          _$AppDatabase,
          $ComposerDraftsTable,
          ComposerDraft,
          $$ComposerDraftsTableFilterComposer,
          $$ComposerDraftsTableOrderingComposer,
          $$ComposerDraftsTableAnnotationComposer,
          $$ComposerDraftsTableCreateCompanionBuilder,
          $$ComposerDraftsTableUpdateCompanionBuilder,
          (
            ComposerDraft,
            BaseReferences<_$AppDatabase, $ComposerDraftsTable, ComposerDraft>,
          ),
          ComposerDraft,
          PrefetchHooks Function()
        > {
  $$ComposerDraftsTableTableManager(
    _$AppDatabase db,
    $ComposerDraftsTable table,
  ) : super(
        TableManagerState(
          db: db,
          table: table,
          createFilteringComposer: () =>
              $$ComposerDraftsTableFilterComposer($db: db, $table: table),
          createOrderingComposer: () =>
              $$ComposerDraftsTableOrderingComposer($db: db, $table: table),
          createComputedFieldComposer: () =>
              $$ComposerDraftsTableAnnotationComposer($db: db, $table: table),
          updateCompanionCallback:
              ({
                Value<String> sessionId = const Value.absent(),
                Value<String> content = const Value.absent(),
                Value<bool> planMode = const Value.absent(),
                Value<String?> effort = const Value.absent(),
                Value<String?> permissionMode = const Value.absent(),
                Value<DateTime> updatedAt = const Value.absent(),
                Value<int> rowid = const Value.absent(),
              }) => ComposerDraftsCompanion(
                sessionId: sessionId,
                content: content,
                planMode: planMode,
                effort: effort,
                permissionMode: permissionMode,
                updatedAt: updatedAt,
                rowid: rowid,
              ),
          createCompanionCallback:
              ({
                required String sessionId,
                required String content,
                Value<bool> planMode = const Value.absent(),
                Value<String?> effort = const Value.absent(),
                Value<String?> permissionMode = const Value.absent(),
                required DateTime updatedAt,
                Value<int> rowid = const Value.absent(),
              }) => ComposerDraftsCompanion.insert(
                sessionId: sessionId,
                content: content,
                planMode: planMode,
                effort: effort,
                permissionMode: permissionMode,
                updatedAt: updatedAt,
                rowid: rowid,
              ),
          withReferenceMapper: (p0) => p0
              .map((e) => (e.readTable(table), BaseReferences(db, table, e)))
              .toList(),
          prefetchHooksCallback: null,
        ),
      );
}

typedef $$ComposerDraftsTableProcessedTableManager =
    ProcessedTableManager<
      _$AppDatabase,
      $ComposerDraftsTable,
      ComposerDraft,
      $$ComposerDraftsTableFilterComposer,
      $$ComposerDraftsTableOrderingComposer,
      $$ComposerDraftsTableAnnotationComposer,
      $$ComposerDraftsTableCreateCompanionBuilder,
      $$ComposerDraftsTableUpdateCompanionBuilder,
      (
        ComposerDraft,
        BaseReferences<_$AppDatabase, $ComposerDraftsTable, ComposerDraft>,
      ),
      ComposerDraft,
      PrefetchHooks Function()
    >;

class $AppDatabaseManager {
  final _$AppDatabase _db;
  $AppDatabaseManager(this._db);
  $$AppKvTableTableManager get appKv =>
      $$AppKvTableTableManager(_db, _db.appKv);
  $$SessionLabelsTableTableManager get sessionLabels =>
      $$SessionLabelsTableTableManager(_db, _db.sessionLabels);
  $$ComposerDraftsTableTableManager get composerDrafts =>
      $$ComposerDraftsTableTableManager(_db, _db.composerDrafts);
}
