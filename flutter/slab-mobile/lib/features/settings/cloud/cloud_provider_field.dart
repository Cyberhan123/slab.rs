/// Dedicated editor for `providers.registry`: configured-provider rows +
/// add/edit sheet (grouped family picker, validation, family prefill of
/// displayName/apiBase/apiKeyEnv). Edits serialize the registry array back
/// through the autosave pipeline. Port of `cloud-provider-field.tsx`.
library;

import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import '../../../data/settings_types.dart';
import '../../../l10n/catalog.dart';
import '../../../theme/slab_tokens.g.dart';
import '../settings_cubit.dart';
import 'cloud_provider_kinds.dart';

final _idPattern = RegExp(r'^[a-z0-9][a-z0-9-_]*$');
final _apiBasePattern = RegExp(r'^https?://.+');

List<Map<String, Object?>> _decodeProviders(Object? value) {
  if (value is List) {
    return value.whereType<Map<String, Object?>>().map((entry) => Map<String, Object?>.of(entry)).toList();
  }
  return [];
}

Map<String, Object?> _defaultProvider(CloudProviderKind kind) => {
      'id': '',
      'family': kind.value,
      'display_name': kind.label,
      'api_base': kind.defaultApiBase,
      'auth': {'api_key': null, 'api_key_env': kind.defaultKeyEnv.isEmpty ? null : kind.defaultKeyEnv},
    };

class CloudProviderField extends StatelessWidget {
  const CloudProviderField({super.key, required this.property, required this.catalog});

  final SettingPropertyView property;
  final SlabCatalog catalog;

  @override
  Widget build(BuildContext context) {
    final cubit = context.read<SettingsCubit>();
    final providers = _decodeProviders(_effectiveValue(cubit));
    final t = catalog.t;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          providers.isEmpty
              ? t('pages.settings.providerRegistry.empty')
              : t('pages.settings.providerRegistry.configuredProviders_other', {'count': '${providers.length}'}),
          style: TextStyle(fontSize: SlabMetrics.textCaption, color: context.tTheme.textColorSecondary),
        ),
        for (final (index, provider) in providers.indexed)
          TCell(
            title: Text((provider['display_name'] ?? provider['id'] ?? '').toString()),
            subtitle: Text(kindForFamily((provider['family'] ?? '').toString()).label,
                style: const TextStyle(fontSize: 11)),
            arrow: true,
            onTap: () => _editProvider(context, providers, index),
          ),
        const SizedBox(height: 4),
        TButton(
          size: TButtonSize.small,
          variant: TButtonVariant.outline,
          onPressed: () => _editProvider(context, providers, null),
          child: Text(t('pages.settings.providerRegistry.addProvider')),
        ),
      ],
    );
  }

  Object? _effectiveValue(SettingsCubit cubit) {
    final draft = cubit.draftTextFor(property).trim();
    if (draft.isNotEmpty) {
      try {
        return jsonDecode(draft);
      } catch (_) {
        // fall through to the effective value
      }
    }
    return property.effectiveValue;
  }

  Future<void> _editProvider(
    BuildContext context,
    List<Map<String, Object?>> providers,
    int? editIndex,
  ) async {
    final t = catalog.t;
    final existing = editIndex != null ? providers[editIndex] : null;
    final family = (existing?['family'] as String?) ?? openAiCompatibleValue;
    final saved = await showModalBottomSheet<Map<String, Object?>>(
      context: context,
      isScrollControlled: true,
      builder: (sheetContext) => Padding(
        padding: EdgeInsets.only(bottom: MediaQuery.of(sheetContext).viewInsets.bottom),
        child: _ProviderSheet(
          title: editIndex == null
              ? t('pages.settings.providerRegistry.dialog.addTitle')
              : t('pages.settings.providerRegistry.dialog.editTitle'),
          provider: existing ?? _defaultProvider(kindForFamily(family)),
          catalog: catalog,
        ),
      ),
    );
    if (saved == null) return;
    if (!context.mounted) return;

    final next = <Map<String, Object?>>[
      for (final provider in providers) Map<String, Object?>.of(provider),
    ];
    if (editIndex == null) {
      next.add(saved);
    } else {
      next[editIndex] = saved;
    }
    context.read<SettingsCubit>().editField(
          property,
          const JsonEncoder.withIndent('  ').convert(next),
        );
  }
}

/// Add/edit sheet with validation + family-driven prefill.
class _ProviderSheet extends StatefulWidget {
  const _ProviderSheet({required this.title, required this.provider, required this.catalog});

  final String title;
  final Map<String, Object?> provider;
  final SlabCatalog catalog;

  @override
  State<_ProviderSheet> createState() => _ProviderSheetState();
}

class _ProviderSheetState extends State<_ProviderSheet> {
  late final Map<String, Object?> _provider = Map<String, Object?>.of(widget.provider);
  late final Map<String, Object?> _auth = Map<String, Object?>.of(
      _provider['auth'] is Map<String, Object?> ? _provider['auth']! as Map<String, Object?> : <String, Object?>{});
  String? _error;

  String get _id => (_provider['id'] ?? '').toString();
  String get _family => (_provider['family'] ?? '').toString();
  String get _displayName => (_provider['display_name'] ?? '').toString();
  String get _apiBase => (_provider['api_base'] ?? '').toString();
  String get _apiKey => (_auth['api_key'] ?? '').toString();
  String get _apiKeyEnv => (_auth['api_key_env'] ?? '').toString();

  void _set(void Function() mutate) => setState(mutate);

  void _onFamilyChanged(String family) {
    final kind = kindForFamily(family);
    _set(() {
      _provider['family'] = family;
      // Prefill UX hints when fields are empty or still the previous kind's
      // defaults (desktop parity: only prefill, never clobber user text).
      if (_displayName.isEmpty) _provider['display_name'] = kind.label;
      if (_apiBase.isEmpty) _provider['api_base'] = kind.defaultApiBase;
      if (_apiKeyEnv.isEmpty && kind.defaultKeyEnv.isNotEmpty) _auth['api_key_env'] = kind.defaultKeyEnv;
    });
  }

  bool _validate() {
    final t = widget.catalog.t;
    if (!_idPattern.hasMatch(_id.trim())) return _fail('Provider ID must match ${_idPattern.pattern}');
    if (_displayName.trim().isEmpty) return _fail(t('pages.settings.field.required'));
    final base = _apiBase.trim();
    if (base.isNotEmpty && !_apiBasePattern.hasMatch(base)) {
      return _fail('API Base URL must start with http:// or https://');
    }
    return true;
  }

  bool _fail(String message) {
    _set(() => _error = message);
    return false;
  }

  void _save() {
    if (!_validate()) return;
    final cleaned = <String, Object?>{
      ..._provider,
      'id': _id.trim(),
      'display_name': _displayName.trim(),
      'api_base': _apiBase.trim(),
      'auth': {
        if (_apiKey.trim().isNotEmpty) 'api_key': _apiKey.trim(),
        if (_apiKeyEnv.trim().isNotEmpty) 'api_key_env': _apiKeyEnv.trim(),
      },
    };
    Navigator.of(context).pop(cleaned);
  }

  @override
  Widget build(BuildContext context) {
    final t = widget.catalog.t;
    final td = context.tTheme;
    return SafeArea(
      child: SingleChildScrollView(
        padding: const EdgeInsets.all(16),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(widget.title, style: const TextStyle(fontSize: 15, fontWeight: FontWeight.w700)),
            const SizedBox(height: 12),
            Text(t('pages.settings.providerRegistry.fields.family.label'),
                style: const TextStyle(fontSize: 12, fontWeight: FontWeight.w600)),
            TCell(
              title: Text(kindForFamily(_family).label),
              arrow: true,
              onTap: () => _pickFamily(context),
            ),
            const SizedBox(height: 8),
            _field(
              label: t('pages.settings.providerRegistry.fields.id.label'),
              value: _id,
              onChanged: (v) => _set(() => _provider['id'] = v),
            ),
            _field(
              label: t('pages.settings.providerRegistry.fields.displayName.label'),
              value: _displayName,
              onChanged: (v) => _set(() => _provider['display_name'] = v),
            ),
            _field(
              label: t('pages.settings.providerRegistry.fields.apiBase.label'),
              value: _apiBase,
              onChanged: (v) => _set(() => _provider['api_base'] = v),
            ),
            _field(
              label: t('pages.settings.providerRegistry.fields.apiKey.label'),
              value: _apiKey,
              secret: true,
              onChanged: (v) => _set(() => _auth['api_key'] = v),
            ),
            _field(
              label: t('pages.settings.providerRegistry.fields.apiKeyEnv.label'),
              value: _apiKeyEnv,
              onChanged: (v) => _set(() => _auth['api_key_env'] = v),
            ),
            if (_error != null)
              Padding(
                padding: const EdgeInsets.only(top: 6),
                child: Text(_error!, style: TextStyle(fontSize: SlabMetrics.textCaption, color: td.errorNormalColor)),
              ),
            const SizedBox(height: 12),
            SizedBox(
              width: double.infinity,
              child: TButton(
                colorScheme: TButtonColorScheme.primary,
                onPressed: _save,
                child: Text(editIsNew ? t('pages.settings.providerRegistry.dialog.add') : t('pages.settings.providerRegistry.dialog.save')),
              ),
            ),
          ],
        ),
      ),
    );
  }

  bool get editIsNew => _id.isEmpty;

  Future<void> _pickFamily(BuildContext context) async {
    final t = widget.catalog.t;
    final picked = await showModalBottomSheet<String>(
      context: context,
      builder: (sheetContext) => SafeArea(
        child: ListView(
          shrinkWrap: true,
          children: [
            for (final (groupLabel, kinds) in kindsByGroup()) ...[
              Padding(
                padding: const EdgeInsets.fromLTRB(16, 10, 16, 2),
                child: Text(groupLabel,
                    style: TextStyle(fontSize: 11, color: Theme.of(sheetContext).textTheme.bodySmall?.color)),
              ),
              for (final kind in kinds)
                TCell(
                  title: Text(kind.label),
                  arrow: false,
                  onTap: () => Navigator.of(sheetContext).pop(kind.value),
                ),
            ],
            Padding(padding: const EdgeInsets.all(12), child: Text(t('pages.settings.providerRegistry.selectFamily'), style: const TextStyle(fontSize: 11))),
          ],
        ),
      ),
    );
    if (picked != null) _onFamilyChanged(picked);
  }

  Widget _field({
    required String label,
    required String value,
    required void Function(String) onChanged,
    bool secret = false,
  }) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(label, style: const TextStyle(fontSize: 12, fontWeight: FontWeight.w600)),
          const SizedBox(height: 2),
          TInput(
            controller: TextEditingController(text: value),
            obscureText: secret,
            onChanged: onChanged,
          ),
        ],
      ),
    );
  }
}
