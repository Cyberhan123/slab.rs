/// Pure settings-form helpers: draft coercion/validation, request bodies,
/// per-type autosave delays, search matching, and structured-error
/// extraction. Port of the desktop `utils.ts` / `use-settings-autosave.ts`
/// pure halves.
library;

import 'dart:convert';

import '../../../data/settings_types.dart';

final _integerPattern = RegExp(r'^-?\d+$');
final _unsignedPattern = RegExp(r'^\d+$');
final _floatPattern = RegExp(r'^-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?$');

/// Outcome of coercing a draft text against a property's schema.
sealed class DraftParse {
  const DraftParse();
}

/// Empty input → reset to default (`op: unset`).
final class DraftUnset extends DraftParse {
  const DraftUnset();
}

final class DraftValue extends DraftParse {
  const DraftValue(this.value);
  final Object? value;
}

/// Coercion failed — [message] is user-facing.
final class DraftInvalid extends DraftParse {
  const DraftInvalid(this.message);
  final String message;
}

/// Coerce [text] (the raw editor input) into the typed wire value.
DraftParse parseDraftValue(SettingPropertyView property, String text) {
  final trimmed = text.trim();
  final schema = property.schema;
  final isJsonish = schema.valueType == SettingValueType.array ||
      schema.valueType == SettingValueType.object ||
      schema.valueType == SettingValueType.taggedUnion ||
      schema.multiline;
  if (isJsonish || schema.valueType == SettingValueType.string && schema.jsonSchema != null) {
    if (trimmed.isEmpty) return const DraftUnset();
    // Complex values edit as JSON text.
    // (enum strings and plain strings fall through to the string branch.)
  }

  switch (schema.valueType) {
    case SettingValueType.boolean:
      if (trimmed.isEmpty) return const DraftUnset();
      return trimmed == 'true' ? const DraftValue(true) : const DraftValue(false);
    case SettingValueType.integer:
      if (trimmed.isEmpty) return const DraftUnset();
      if (!_integerPattern.hasMatch(trimmed)) {
        return const DraftInvalid('Enter a whole number (e.g. 42 or -7).');
      }
      return DraftValue(int.parse(trimmed));
    case SettingValueType.unsigned:
      if (trimmed.isEmpty) return const DraftUnset();
      if (!_unsignedPattern.hasMatch(trimmed)) {
        return const DraftInvalid('Enter a non-negative whole number (e.g. 8).');
      }
      return DraftValue(int.parse(trimmed));
    case SettingValueType.float:
      if (trimmed.isEmpty) return const DraftUnset();
      if (!_floatPattern.hasMatch(trimmed)) {
        return const DraftInvalid('Enter a number (e.g. 0.5 or -1.2e3).');
      }
      return DraftValue(double.parse(trimmed));
    default:
      break;
  }

  // String-ish fields (plain / enum / JSON text).
  final isJsonText = schema.valueType == SettingValueType.array ||
      schema.valueType == SettingValueType.object ||
      schema.valueType == SettingValueType.taggedUnion ||
      schema.multiline;
  if (isJsonText && trimmed.isNotEmpty) {
    final decoded = _tryJson(trimmed);
    if (decoded == null) {
      return const DraftInvalid('Enter valid JSON.');
    }
    return DraftValue(decoded);
  }
  if (trimmed.isEmpty) {
    // Plain strings: empty means unset (reset to default), desktop parity.
    return const DraftUnset();
  }
  return DraftValue(trimmed);
}

Object? _tryJson(String text) {
  try {
    return jsonDecode(text);
  } catch (_) {
    return null;
  }
}

/// Autosave debounce per schema shape (desktop parity: fast toggles, slow
/// JSON, medium everything else).
Duration autoSaveDelay(SettingPropertySchema schema) {
  if (schema.valueType == SettingValueType.boolean || schema.enumValues != null) {
    return const Duration(milliseconds: 150);
  }
  if (schema.valueType == SettingValueType.array ||
      schema.valueType == SettingValueType.object ||
      schema.valueType == SettingValueType.taggedUnion ||
      schema.multiline) {
    return const Duration(milliseconds: 900);
  }
  return const Duration(milliseconds: 650);
}

/// Whether a property matches the search across label, pmid, description,
/// and the server-provided search terms.
bool searchMatchesProperty(SettingPropertyView property, String query) {
  final q = query.trim().toLowerCase();
  if (q.isEmpty) return true;
  if (property.label.toLowerCase().contains(q)) return true;
  if (property.pmid.toLowerCase().contains(q)) return true;
  if (property.descriptionMd.toLowerCase().contains(q)) return true;
  return property.searchTerms.any((term) => term.toLowerCase().contains(q));
}

bool searchMatchesSection(SettingsSectionView section, String query) {
  final q = query.trim().toLowerCase();
  if (q.isEmpty) return true;
  if (section.title.toLowerCase().contains(q) || section.id.toLowerCase().contains(q)) return true;
  return section.subsections.any(
      (sub) => sub.properties.any((property) => searchMatchesProperty(property, q)));
}

/// Extract `(pointerPath → message)` pairs from a validation error body's
/// structured `data` when present; the flat message under `''`.
Map<String, String> extractStructuredError(Object? body) {
  if (body is! Map<String, Object?>) return const {};
  final message = body['message'] is String ? body['message']! as String : '';
  final path = body['path'] is String && (body['path']! as String).isNotEmpty ? body['path']! as String : '';
  if (message.isEmpty) return const {};
  return {path: message};
}
