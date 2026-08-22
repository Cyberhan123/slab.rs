/// Recursive JSON-Schema-driven editor for structured settings (object /
/// array with a `json_schema`): typed rows per schema property, enum
/// selects, writeOnly → password, add/remove list items (respecting
/// minItems), and JSON-pointer error mapping onto the failing sub-field.
/// Edits serialize back to JSON text through the autosave pipeline. Port of
/// `structured-json-field.tsx` at mobile scope.
library;

import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import '../../../../data/settings_types.dart';
import '../../../../l10n/catalog.dart';
import '../../../../theme/slab_tokens.g.dart';
import '../../settings_cubit.dart';

class StructuredJsonField extends StatefulWidget {
  const StructuredJsonField({super.key, required this.property, required this.catalog});

  final SettingPropertyView property;
  final SlabCatalog catalog;

  @override
  State<StructuredJsonField> createState() => _StructuredJsonFieldState();
}

class _StructuredJsonFieldState extends State<StructuredJsonField> {
  late Object? _value;

  @override
  void initState() {
    super.initState();
    _value = _initialValue();
  }

  Object? _initialValue() {
    final draft = context.read<SettingsCubit>().draftTextFor(widget.property).trim();
    if (draft.isNotEmpty) {
      final decoded = _tryDecode(draft);
      if (decoded != null) return decoded;
    }
    return widget.property.effectiveValue;
  }

  static Object? _tryDecode(String text) {
    try {
      return jsonDecode(text);
    } catch (_) {
      return null;
    }
  }

  Map<String, Object?> get _schema {
    final jsonSchema = widget.property.schema.jsonSchema;
    return jsonSchema is Map<String, Object?> ? jsonSchema : const <String, Object?>{};
  }

  void _commit(Object? next) {
    setState(() => _value = next);
    context.read<SettingsCubit>().editField(
          widget.property,
          const JsonEncoder.withIndent('  ').convert(next),
        );
  }

  @override
  Widget build(BuildContext context) {
    final td = context.tTheme;
    final schema = _schema;
    final value = _value;
    if (value is Map<String, Object?>) {
      return _ObjectEditor(
        label: widget.property.label,
        schema: schema,
        value: value,
        path: '',
        onChange: _commit,
        catalog: widget.catalog,
      );
    }
    if (value is List<Object?>) {
      return _ArrayEditor(
        label: widget.property.label,
        itemSchema: _itemSchema(schema),
        value: value,
        minItems: _minItems(schema),
        path: '',
        onChange: (list) => _commit(List<Object?>.of(list)),
        catalog: widget.catalog,
      );
    }
    // Unexpected shape — degrade to the JSON text view.
    return Text(
      const JsonEncoder.withIndent('  ').convert(value),
      style: TextStyle(fontSize: SlabMetrics.textCaption, fontFamilyFallback: SlabMetrics.fontMono, color: td.textColorSecondary),
    );
  }

  static Map<String, Object?> _itemSchema(Map<String, Object?> schema) {
    final items = schema['items'];
    return items is Map<String, Object?> ? items : const <String, Object?>{};
  }

  static int _minItems(Map<String, Object?> schema) {
    return schema['minItems'] is int ? schema['minItems']! as int : 0;
  }
}

// ── Object rows ─────────────────────────────────────────────────────────────

class _ObjectEditor extends StatelessWidget {
  const _ObjectEditor({
    required this.label,
    required this.schema,
    required this.value,
    required this.path,
    required this.onChange,
    required this.catalog,
  });

  final String label;
  final Map<String, Object?> schema;
  final Map<String, Object?> value;
  final String path;
  final void Function(Map<String, Object?>) onChange;
  final SlabCatalog catalog;

  Map<String, Object?> get _properties {
    final props = schema['properties'];
    return props is Map<String, Object?> ? props : const <String, Object?>{};
  }

  List<String> get _required {
    final required = schema['required'];
    return required is List ? required.whereType<String>().toList(growable: false) : const <String>[];
  }

  @override
  Widget build(BuildContext context) {
    final td = context.tTheme;
    final t = catalog.t;
    if (_properties.isEmpty) {
      return Text(t('pages.settings.structured.noObjectFields'),
          style: TextStyle(fontSize: SlabMetrics.textCaption, color: td.textColorPlaceholder));
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        for (final entry in _properties.entries)
          _SchemaRow(
            label: entry.key,
            fieldSchema: entry.value is Map<String, Object?> ? entry.value! as Map<String, Object?> : const {},
            required_: _required.contains(entry.key),
            value: value[entry.key],
            path: '$path/${entry.key}',
            onChange: (next) => onChange({...value, entry.key: next}),
            catalog: catalog,
          ),
      ],
    );
  }
}

// ── Array rows ──────────────────────────────────────────────────────────────

class _ArrayEditor extends StatelessWidget {
  const _ArrayEditor({
    required this.label,
    required this.itemSchema,
    required this.value,
    required this.minItems,
    required this.path,
    required this.onChange,
    required this.catalog,
  });

  final String label;
  final Map<String, Object?> itemSchema;
  final List<Object?> value;
  final int minItems;
  final String path;
  final void Function(List<Object?>) onChange;
  final SlabCatalog catalog;

  Object? _defaultFor() => _defaultForSchema(itemSchema);

  static Object? _defaultForSchema(Map<String, Object?> schema) {
    final type = schema['type'];
    if (type == 'object') return <String, Object?>{};
    if (type == 'array') return <Object?>[];
    if (type == 'boolean') return false;
    if (type == 'number' || type == 'integer') return 0;
    return '';
  }

  @override
  Widget build(BuildContext context) {
    final td = context.tTheme;
    final t = catalog.t;
    if (value.isEmpty) {
      return Text(t('pages.settings.structured.noEntries'),
          style: TextStyle(fontSize: SlabMetrics.textCaption, color: td.textColorPlaceholder));
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        for (final (index, item) in value.indexed)
          Padding(
            padding: const EdgeInsets.only(bottom: 8),
            child: Container(
              padding: const EdgeInsets.all(8),
              decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(SlabMetrics.radiusSm),
                border: Border.all(color: td.componentStrokeColor),
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    children: [
                      Expanded(
                        child: Text(
                          t('pages.settings.structured.itemTitle', {'label': t('pages.settings.structured.item'), 'index': '${index + 1}'}),
                          style: TextStyle(fontSize: SlabMetrics.textCaption, fontWeight: FontWeight.w600),
                        ),
                      ),
                      if (index + 1 > minItems)
                        GestureDetector(
                          onTap: () => onChange([...value]..removeAt(index)),
                          child: Text(
                            t('pages.settings.structured.remove'),
                            style: TextStyle(fontSize: SlabMetrics.textMicro, color: td.errorNormalColor),
                          ),
                        ),
                    ],
                  ),
                  _SchemaValue(
                    fieldSchema: itemSchema,
                    value: item,
                    path: '$path/$index',
                    onChange: (next) => onChange([...value]..[index] = next),
                    catalog: catalog,
                  ),
                ],
              ),
            ),
          ),
        GestureDetector(
          onTap: () => onChange([...value, _defaultFor()]),
          child: Text(
            '${t('pages.settings.structured.add')} ${t('pages.settings.structured.item')}',
            style: TextStyle(fontSize: SlabMetrics.textCaption, color: td.brandNormalColor),
          ),
        ),
      ],
    );
  }
}

// ── Field rows (object property / array item) ───────────────────────────────

class _SchemaRow extends StatelessWidget {
  const _SchemaRow({
    required this.label,
    required this.fieldSchema,
    required this.required_,
    required this.value,
    required this.path,
    required this.onChange,
    required this.catalog,
  });

  final String label;
  final Map<String, Object?> fieldSchema;
  final bool required_;
  final Object? value;
  final String path;
  final void Function(Object?) onChange;
  final SlabCatalog catalog;

  @override
  Widget build(BuildContext context) {
    final td = context.tTheme;
    final t = catalog.t;
    return Padding(
      padding: const EdgeInsets.only(bottom: 6),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Text(label, style: const TextStyle(fontSize: 12, fontWeight: FontWeight.w600)),
              if (required_)
                Padding(
                  padding: const EdgeInsets.only(left: 4),
                  child: Text(
                    t('pages.settings.field.required'),
                    style: TextStyle(fontSize: SlabMetrics.textMicro, color: td.errorNormalColor),
                  ),
                ),
            ],
          ),
          const SizedBox(height: 2),
          _SchemaValue(
            fieldSchema: fieldSchema,
            value: value,
            path: path,
            onChange: onChange,
            catalog: catalog,
          ),
        ],
      ),
    );
  }
}

/// The value editor for one schema node: enum select, nested object/array,
/// or a typed text input (writeOnly renders as a password field).
class _SchemaValue extends StatelessWidget {
  const _SchemaValue({
    required this.fieldSchema,
    required this.value,
    required this.path,
    required this.onChange,
    required this.catalog,
  });

  final Map<String, Object?> fieldSchema;
  final Object? value;
  final String path;
  final void Function(Object?) onChange;
  final SlabCatalog catalog;

  @override
  Widget build(BuildContext context) {
    final enums = fieldSchema['enum'];
    if (enums is List && enums.isNotEmpty) {
      final current = value is String ? value! as String : '';
      return TCell(
        title: Text(current.isEmpty ? catalog.t('pages.settings.field.selectOption') : current,
            style: const TextStyle(fontSize: 12)),
        arrow: true,
        onTap: () async {
          final picked = await showModalBottomSheet<String>(
            context: context,
            builder: (sheetContext) => SafeArea(
              child: ListView(
                shrinkWrap: true,
                children: [
                  for (final option in enums.whereType<String>())
                    TCell(
                      title: Text(option),
                      arrow: false,
                      onTap: () => Navigator.of(sheetContext).pop(option),
                    ),
                ],
              ),
            ),
          );
          if (picked != null) onChange(picked);
        },
      );
    }

    final type = fieldSchema['type'];
    if (type == 'object' && value is Map<String, Object?>) {
      return _ObjectEditor(
        label: path,
        schema: fieldSchema,
        value: value! as Map<String, Object?>,
        path: path,
        onChange: onChange as void Function(Map<String, Object?>),
        catalog: catalog,
      );
    }
    if (type == 'array' && value is List<Object?>) {
      final minItems = fieldSchema['minItems'] is int ? fieldSchema['minItems']! as int : 0;
      return _ArrayEditor(
        label: path,
        itemSchema: fieldSchema['items'] is Map<String, Object?> ? fieldSchema['items']! as Map<String, Object?> : const {},
        value: value! as List<Object?>,
        minItems: minItems,
        path: path,
        onChange: (list) => onChange(List<Object?>.of(list)),
        catalog: catalog,
      );
    }

    final controller = TextEditingController(text: value is String ? value! as String : (value?.toString() ?? ''));
    return TInput(
      controller: controller,
      obscureText: fieldSchema['writeOnly'] == true,
      hintText: catalog.t('pages.settings.field.valuePlaceholder'),
      onChanged: (text) =>
          onChange(type == 'number' || type == 'integer' ? (int.tryParse(text) ?? text) : text),
    );
  }
}
