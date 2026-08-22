/// Assistant model selection state: the chat-capable model list, the current
/// selection, and the prepare pipeline (download → load with one retry).
/// The pending-switch dialog state (keep vs new session) also lives here.
library;

import 'package:flutter_bloc/flutter_bloc.dart';

import 'package:slab_mobile/data/rest/model_types.dart';
import 'model_repository.dart';

class ModelState {
  const ModelState({
    this.models = const [],
    this.loading = false,
    this.selectedId,
    this.loadedContextLength,
    this.preparing = false,
    this.prepareError,
    this.pendingSwitchTo,
  });

  final List<AiModelRecord> models;
  final bool loading;
  final String? selectedId;

  /// Runtime context length of the currently loaded model (usage % driver).
  final int? loadedContextLength;
  final bool preparing;
  final String? prepareError;

  /// Non-null while the "switch model on a populated session?" dialog is
  /// open — the model id the user picked.
  final String? pendingSwitchTo;

  AiModelRecord? get selected => models.where((model) => model.id == selectedId).firstOrNull;

  ModelState copyWith({
    List<AiModelRecord>? models,
    bool? loading,
    String? selectedId,
    bool clearSelectedId = false,
    int? loadedContextLength,
    bool? preparing,
    String? prepareError,
    bool clearPrepareError = false,
    String? pendingSwitchTo,
    bool clearPendingSwitch = false,
  }) =>
      ModelState(
        models: models ?? this.models,
        loading: loading ?? this.loading,
        selectedId: clearSelectedId ? null : (selectedId ?? this.selectedId),
        loadedContextLength: loadedContextLength ?? this.loadedContextLength,
        preparing: preparing ?? this.preparing,
        prepareError: clearPrepareError ? null : (prepareError ?? this.prepareError),
        pendingSwitchTo: clearPendingSwitch ? null : (pendingSwitchTo ?? this.pendingSwitchTo),
      );
}

class ModelCubit extends Cubit<ModelState> {
  ModelCubit({required ModelRepository repository})
      : _repository = repository,
        super(const ModelState());

  final ModelRepository _repository;

  /// Load the model list and settle a default selection (current selection
  /// wins; otherwise the first chat-capable model).
  Future<void> load({String? preferredId}) async {
    emit(state.copyWith(loading: true, clearPrepareError: true));
    try {
      final models = await _repository.chatModels();
      var selectedId = preferredId ?? state.selectedId;
      final stillThere = models.any((model) => model.id == selectedId);
      if (!stillThere) {
        selectedId = models.isNotEmpty ? models.first.id : null;
      }
      final loaded = models.where((model) => model.id == selectedId).firstOrNull;
      emit(state.copyWith(
        models: models,
        loading: false,
        selectedId: selectedId,
        clearSelectedId: selectedId == null,
        loadedContextLength: loaded?.runtimeContextLength,
      ));
    } on Object catch (error) {
      emit(state.copyWith(loading: false, prepareError: error.toString()));
    }
  }

  /// Straight selection (empty session / explicit apply after dialog).
  Future<void> select(String modelId) async {
    emit(state.copyWith(selectedId: modelId, clearPendingSwitch: true));
    await _prepareIfLocal(modelId);
  }

  /// User picked [modelId] in the picker; the dialog decision comes via
  /// [applyPendingSwitch] / [cancelPendingSwitch].
  void requestSwitch(String modelId) {
    emit(state.copyWith(pendingSwitchTo: modelId));
  }

  void cancelPendingSwitch() {
    emit(state.copyWith(clearPendingSwitch: true));
  }

  /// Apply the pending switch to the CURRENT session.
  Future<void> applyPendingSwitch() async {
    final target = state.pendingSwitchTo;
    if (target == null) return;
    emit(state.copyWith(clearPendingSwitch: true));
    await select(target);
  }

  /// Download + load the selected local model (cloud models are no-ops).
  /// Surfaces failures via `prepareError`; one force re-download retry rides
  /// inside the repository.
  Future<void> _prepareIfLocal(String modelId) async {
    final model = state.models.where((m) => m.id == modelId).firstOrNull;
    if (model == null || model.kind == 'cloud') return;
    if (!model.downloaded || model.runtimeContextLength == null) {
      emit(state.copyWith(preparing: true, clearPrepareError: true));
      try {
        final prepared = await _repository.prepare(modelId);
        emit(state.copyWith(preparing: false, loadedContextLength: prepared.runtimeContextLength));
        // Refresh the list so downloaded/loaded flags reflect reality.
        await load(preferredId: modelId);
      } on Object catch (error) {
        emit(state.copyWith(preparing: false, prepareError: error.toString()));
      }
    }
  }
}
