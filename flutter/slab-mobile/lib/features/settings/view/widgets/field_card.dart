/// One property card: label + effect/override/status badges + reset, and
/// the editor dispatched by schema type (switch / input / select / textarea /
/// structured JSON / provider registry). Port of `setting-field-card.tsx`.
library;

import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import 'package:slab_mobile/data/rest/settings_types.dart';
import 'package:slab_mobile/core/l10n/catalog.dart';
import 'package:slab_mobile/core/theme/slab_tokens.g.dart';
import '../../settings_cubit.dart';
import '../../cloud/cloud_provider_field.dart';
import 'structured_json_field.dart';

class SettingFieldCard extends StatelessWidget {
  const SettingFieldCard({super.key, required this.property, required this.catalog});

  final SettingPropertyView property;
  final SlabCatalog catalog;

  @override
  Widget build(BuildContext context) {
    final td = context.tTheme;
    final t = catalog.t;
    final cubit = context.read<SettingsCubit>();
    final status = context.select((SettingsCubit c) => c.state.fieldStatus[property.pmid]);
    final error = context.select((SettingsCubit c) => c.state.fieldErrors[property.pmid]);

    return Container(
      margin: const EdgeInsets.symmetric(vertical: 4, horizontal: 8),
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: td.bgColorSecondaryContainer,
        borderRadius: BorderRadius.circular(SlabMetrics.radiusMd),
        border: Border.all(color: status == FieldStatus.error ? td.errorNormalColor : td.componentStrokeColor),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Expanded(
                child: Text(
                  property.label,
                  style: const TextStyle(fontSize: 14, fontWeight: FontWeight.w600),
                ),
              ),
              if (property.changeEffect != SettingChangeEffect.none)
                _badge(
                  context,
                  label: switch (property.changeEffect) {
                    SettingChangeEffect.live => t('pages.settings.effect.live'),
                    SettingChangeEffect.needsRestart => t('pages.settings.effect.needsRestart'),
                    SettingChangeEffect.needsModelReload => t('pages.settings.effect.needsModelReload'),
                    _ => '',
                  },
                  color: property.changeEffect == SettingChangeEffect.live ? td.successNormalColor : td.warningNormalColor,
                ),
              const SizedBox(width: 6),
              _statusBadge(context, status, t),
            ],
          ),
          if (property.descriptionMd.isNotEmpty)
            Padding(
              padding: const EdgeInsets.only(top: 2),
              child: Text(
                property.descriptionMd,
                style: TextStyle(fontSize: SlabMetrics.textCaption, height: 1.4, color: td.textColorSecondary),
              ),
            ),
          if (property.overriddenBy case final override?)
            Padding(
              padding: const EdgeInsets.only(top: 2),
              child: Text(
                switch (override) {
                  SettingEnvOverride(:final varName, :final varValuePresent) => varValuePresent
                      ? t('pages.settings.effect.envPresent', {'varName': varName})
                      : t('pages.settings.effect.envMissing', {'varName': varName}),
                  SettingParentOverride(:final pmid) =>
                    t('pages.settings.effect.inheritedFrom', {'pmid': pmid}),
                },
                style: TextStyle(fontSize: SlabMetrics.textMicro, color: td.textColorPlaceholder),
              ),
            ),
          const SizedBox(height: 8),
          if (property.editable) _editor(context, cubit) else _readOnlyValue(context),
          if (error != null)
            Padding(
              padding: const EdgeInsets.only(top: 4),
              child: Text(
                error,
                style: TextStyle(fontSize: SlabMetrics.textCaption, color: td.errorNormalColor),
              ),
            ),
          if (property.editable)
            Align(
              alignment: Alignment.centerRight,
              child: TextButton(
                onPressed: (property.isOverridden ||
                        context.select((SettingsCubit c) => c.state.drafts.containsKey(property.pmid)))
                    ? () async => cubit.resetField(property)
                    : null,
                child: Text(
                  t('pages.settings.field.reset'),
                  style: TextStyle(fontSize: SlabMetrics.textCaption),
                ),
              ),
            ),
        ],
      ),
    );
  }

  Widget _readOnlyValue(BuildContext context) {
    final value = context.read<SettingsCubit>().draftTextFor(property);
    return Text(
      value.isEmpty ? '—' : value,
      style: TextStyle(
        fontSize: SlabMetrics.textCaption,
        fontFamilyFallback: SlabMetrics.fontMono,
        color: context.tTheme.textColorSecondary,
      ),
    );
  }

  Widget _editor(BuildContext context, SettingsCubit cubit) {
    final schema = property.schema;
    final t = catalog.t;

    // Provider registry gets its dedicated editor.
    if (property.pmid == 'providers.registry') {
      return CloudProviderField(property: property, catalog: catalog);
    }

    if (schema.valueType == SettingValueType.boolean) {
      final current = cubit.draftTextFor(property) == 'true' ||
          (property.effectiveValue == true && !cubit.state.drafts.containsKey(property.pmid));
      return Row(
        children: [
          TSwitch(
            value: current,
            onChanged: (checked) => cubit.editField(property, checked ? 'true' : 'false'),
          ),
        ],
      );
    }

    if (schema.enumValues != null && schema.valueType == SettingValueType.string) {
      return _EnumEditor(property: property, catalog: catalog);
    }

    final structured = schema.valueType == SettingValueType.array ||
        schema.valueType == SettingValueType.object ||
        schema.valueType == SettingValueType.taggedUnion;
    if (structured && schema.jsonSchema is Map<String, Object?>) {
      return StructuredJsonField(property: property, catalog: catalog);
    }
    if (structured || schema.multiline) {
      return _JsonTextEditor(property: property, placeholder: t('pages.settings.field.jsonPlaceholder'));
    }

    final numeric = schema.valueType == SettingValueType.integer ||
        schema.valueType == SettingValueType.unsigned ||
        schema.valueType == SettingValueType.float;
    return _ScalarEditor(
      property: property,
      numeric: numeric,
      secret: schema.secret,
      placeholder: numeric
          ? (schema.valueType == SettingValueType.float
              ? t('pages.settings.field.numberPlaceholder')
              : t('pages.settings.field.integerPlaceholder'))
          : t('pages.settings.field.valuePlaceholder'),
    );
  }

  Widget _badge(BuildContext context, {required String label, required Color color}) {
    return Theme(
      data: Theme.of(context).copyWith(extensions: [TTagThemeData(isLight: true, textColor: color)]),
      child: TTag(label, size: TTagSize.small),
    );
  }

  Widget _statusBadge(BuildContext context, FieldStatus? status, String Function(String) t) {
    if (status == null) return const SizedBox.shrink();
    final td = context.tTheme;
    final (label, color) = switch (status) {
      FieldStatus.dirty => (t('pages.settings.autosave.waitingAutoSave'), td.textColorPlaceholder),
      FieldStatus.saving => (t('pages.settings.autosave.savingChanges'), td.brandNormalColor),
      FieldStatus.saved => (t('pages.settings.autosave.savedAutomatically'), td.successNormalColor),
      FieldStatus.error => (t('pages.settings.autosave.needsAttention'), td.errorNormalColor),
    };
    return _badge(context, label: label, color: color);
  }
}

/// Enum picker: tap row → bottom sheet of options.
class _EnumEditor extends StatelessWidget {
  const _EnumEditor({required this.property, required this.catalog});

  final SettingPropertyView property;
  final SlabCatalog catalog;

  @override
  Widget build(BuildContext context) {
    final cubit = context.read<SettingsCubit>();
    final current = cubit.draftTextFor(property);
    return TCell(
      title: Text(
        current.isEmpty ? catalog.t('pages.settings.field.selectOption') : current,
        style: TextStyle(fontSize: SlabMetrics.textCaption),
      ),
      onTap: () async {
        final picked = await showModalBottomSheet<String>(
          context: context,
          builder: (sheetContext) => SafeArea(
            child: ListView(
              shrinkWrap: true,
              children: [
                for (final option in property.schema.enumValues ?? const <String>[])
                  TCell(
                    title: Text(option),
                    arrow: false,
                    onTap: () => Navigator.of(sheetContext).pop(option),
                  ),
              ],
            ),
          ),
        );
        if (picked != null) {
          cubit.editField(property, picked);
        }
      },
    );
  }
}

/// Plain scalar input (numeric / text / secret). Controller seeded once per
/// pmid so rebuilds never clobber typing.
class _ScalarEditor extends StatefulWidget {
  const _ScalarEditor({
    required this.property,
    required this.numeric,
    required this.secret,
    required this.placeholder,
  });

  final SettingPropertyView property;
  final bool numeric;
  final bool secret;
  final String placeholder;

  @override
  State<_ScalarEditor> createState() => _ScalarEditorState();
}

class _ScalarEditorState extends State<_ScalarEditor> {
  late final TextEditingController _controller;

  @override
  void initState() {
    super.initState();
    _controller = TextEditingController(text: context.read<SettingsCubit>().draftTextFor(widget.property));
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return TInput(
      controller: _controller,
      hintText: widget.placeholder,
      obscureText: widget.secret,
      inputType: widget.numeric
          ? (widget.property.schema.valueType == SettingValueType.float ? const TextInputType.numberWithOptions(decimal: true) : TextInputType.number)
          : TextInputType.text,
      onChanged: (text) => context.read<SettingsCubit>().editField(widget.property, text),
    );
  }
}

/// JSON text area for complex values without a usable json_schema.
class _JsonTextEditor extends StatefulWidget {
  const _JsonTextEditor({required this.property, required this.placeholder});

  final SettingPropertyView property;
  final String placeholder;

  @override
  State<_JsonTextEditor> createState() => _JsonTextEditorState();
}

class _JsonTextEditorState extends State<_JsonTextEditor> {
  late final TextEditingController _controller;

  @override
  void initState() {
    super.initState();
    _controller = TextEditingController(text: context.read<SettingsCubit>().draftTextFor(widget.property));
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return TTextarea(
      controller: _controller,
      minLines: 2,
      maxLines: 8,
      hintText: widget.placeholder,
      onSubmitted: (_) {},
      onChanged: (text) => context.read<SettingsCubit>().editField(widget.property, text),
    );
  }
}
