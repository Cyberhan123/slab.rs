/// Settings feature state machine: document load, search filter, per-field
/// debounced autosave with the race-free latest-draft check, and the dirty/
/// saving/saved/error status taxonomy that drives the unsaved-changes guard.
/// Port of the desktop `use-settings-autosave.ts` + page state.
library;

import 'dart:async';
import 'dart:convert';

import 'package:flutter_bloc/flutter_bloc.dart';

import '../../data/rest_client.dart';
import '../../data/settings_types.dart';
import 'autosave/request_body.dart';

enum FieldStatus { dirty, saving, saved, error }

class SettingsState {
  const SettingsState({
    this.document,
    this.loading = false,
    this.error,
    this.search = '',
    this.drafts = const {},
    this.fieldStatus = const {},
    this.fieldErrors = const {},
  });

  final SettingsDocumentView? document;
  final bool loading;
  final String? error;
  final String search;

  /// Raw editor text per pmid (null = unchanged from effective value).
  final Map<String, String> drafts;
  final Map<String, FieldStatus> fieldStatus;

  /// Coercion / server errors per pmid (structured sub-field errors render
  /// inside the structured editor via `fieldErrors['pmid|/pointer']`).
  final Map<String, String> fieldErrors;

  int get dirtyCount => fieldStatus.values.where((s) => s == FieldStatus.dirty).length;
  int get savingCount => fieldStatus.values.where((s) => s == FieldStatus.saving).length;
  int get errorCount => fieldStatus.values.where((s) => s == FieldStatus.error).length;

  /// Unsaved-changes guard: any dirty / in-flight / errored field blocks pop.
  bool get hasUnsavedChanges => dirtyCount + savingCount + errorCount > 0;

  SettingsState copyWith({
    SettingsDocumentView? document,
    bool? loading,
    String? error,
    bool clearError = false,
    String? search,
    Map<String, String>? drafts,
    Map<String, FieldStatus>? fieldStatus,
    Map<String, String>? fieldErrors,
  }) =>
      SettingsState(
        document: document ?? this.document,
        loading: loading ?? this.loading,
        error: clearError ? null : (error ?? this.error),
        search: search ?? this.search,
        drafts: drafts ?? this.drafts,
        fieldStatus: fieldStatus ?? this.fieldStatus,
        fieldErrors: fieldErrors ?? this.fieldErrors,
      );
}

class SettingsCubit extends Cubit<SettingsState> {
  SettingsCubit({required SlabRestClient client})
      : _client = client,
        super(const SettingsState());

  final SlabRestClient _client;
  final Map<String, Timer> _timers = {};

  Future<void> load() async {
    emit(state.copyWith(loading: true, clearError: true));
    try {
      final document = await _client.getSettingsDocument();
      emit(state.copyWith(document: document, loading: false));
    } on Object catch (error) {
      emit(state.copyWith(loading: false, error: error.toString()));
    }
  }

  void setSearch(String query) => emit(state.copyWith(search: query));

  /// Seed a draft from the effective value (structured fields serialize to
  /// JSON text; scalars render plain).
  String draftTextFor(SettingPropertyView property) {
    final draft = state.drafts[property.pmid];
    if (draft != null) return draft;
    final value = property.effectiveValue;
    if (value == null) return '';
    if (value is String) return value;
    if (value is bool || value is num) return value.toString();
    return _encodeJson(value);
  }

  String _encodeJson(Object? value) => const JsonEncoder.withIndent('  ').convert(value);

  /// User edited a field: mark dirty, reset its error, (re)schedule the save.
  void editField(SettingPropertyView property, String text) {
    final pmid = property.pmid;
    _timers[pmid]?.cancel();
    emit(state.copyWith(
      drafts: {...state.drafts, pmid: text},
      fieldStatus: {...state.fieldStatus, pmid: FieldStatus.dirty},
      fieldErrors: _withoutError(state.fieldErrors, pmid),
    ));
    _timers[pmid] = Timer(autoSaveDelay(property.schema), () => _saveField(property, text));
  }

  /// Reset-to-default button: unset immediately (no debounce).
  Future<void> resetField(SettingPropertyView property) async {
    _timers[property.pmid]?.cancel();
    await _saveField(property, null, unset: true);
  }

  Future<void> _saveField(SettingPropertyView property, String? text, {bool unset = false}) async {
    final pmid = property.pmid;

    if (!unset && text != null) {
      final parsed = parseDraftValue(property, text);
      switch (parsed) {
        case DraftInvalid(:final message):
          emit(state.copyWith(
            fieldStatus: {...state.fieldStatus, pmid: FieldStatus.error},
            fieldErrors: {...state.fieldErrors, pmid: message},
          ));
          return;
        case DraftUnset():
          unset = true;
        case DraftValue(:final value):
          await _putField(property, set: true, value: value, draftSnapshot: text);
          return;
      }
    }
    await _putField(property, set: !unset, value: null, draftSnapshot: text, unset: unset);
  }

  Future<void> _putField(
    SettingPropertyView property, {
    required bool set,
    Object? value,
    String? draftSnapshot,
    bool unset = false,
  }) async {
    final pmid = property.pmid;
    emit(state.copyWith(
      fieldStatus: {...state.fieldStatus, pmid: FieldStatus.saving},
      fieldErrors: _withoutError(state.fieldErrors, pmid),
    ));
    try {
      final updated = await _client.updateSetting(pmid: pmid, set: set, value: value);
      // Race guard: a newer draft arrived while this request was in flight —
      // keep the dirty state; the newer timer owns the next save.
      final latest = state.drafts[pmid];
      if (draftSnapshot != null && latest != draftSnapshot) {
        emit(state.copyWith(
          document: _patchProperty(state.document, updated),
        ));
        return;
      }
      emit(state.copyWith(
        document: _patchProperty(state.document, updated),
        fieldStatus: {...state.fieldStatus, pmid: FieldStatus.saved},
        drafts: _withoutDraft(state.drafts, pmid),
      ));
    } on SettingValidationException catch (error) {
      emit(state.copyWith(
        fieldStatus: {...state.fieldStatus, pmid: FieldStatus.error},
        fieldErrors: {...state.fieldErrors, pmid: error.data.message},
      ));
    } on Object catch (error) {
      emit(state.copyWith(
        fieldStatus: {...state.fieldStatus, pmid: FieldStatus.error},
        fieldErrors: {...state.fieldErrors, pmid: error.toString()},
      ));
    }
  }

  /// Splice an updated property back into the document (single source).
  SettingsDocumentView? _patchProperty(SettingsDocumentView? document, SettingPropertyView updated) {
    if (document == null) return null;
    return SettingsDocumentView(
      schemaVersion: document.schemaVersion,
      settingsPath: document.settingsPath,
      warnings: document.warnings,
      sections: [
        for (final section in document.sections)
          SettingsSectionView(
            id: section.id,
            title: section.title,
            descriptionMd: section.descriptionMd,
            subsections: [
              for (final subsection in section.subsections)
                SettingsSubsectionView(
                  id: subsection.id,
                  title: subsection.title,
                  descriptionMd: subsection.descriptionMd,
                  properties: [
                    for (final property in subsection.properties)
                      property.pmid == updated.pmid ? updated : property,
                  ],
                ),
            ],
          ),
      ],
    );
  }

  Map<String, String> _withoutError(Map<String, String> errors, String pmid) => {
        for (final entry in errors.entries)
          if (entry.key != pmid && !entry.key.startsWith('$pmid|')) entry.key: entry.value,
      };

  Map<String, String> _withoutDraft(Map<String, String> drafts, String pmid) => {
        for (final entry in drafts.entries)
          if (entry.key != pmid) entry.key: entry.value,
      };

  @override
  Future<void> close() async {
    for (final timer in _timers.values) {
      timer.cancel();
    }
    _timers.clear();
    await super.close();
  }
}
