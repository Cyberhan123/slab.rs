/// Composer: text area + attachments strip + option rows (plan mode,
/// reasoning effort, permission mode) + the `/`-command menu, with
/// slash-command dispatch (control / togglePlan / send) and debounced draft
/// persistence. Port of the desktop `sender.tsx` at mobile scope — the
/// plus-menu hosts the gallery image picker (data-URL attachments); the
/// desktop mic/native-path attachment paths are intentionally absent (voice
/// is deferred; see the plan).
library;

import 'dart:async';
import 'dart:convert';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:get_it/get_it.dart';
import 'package:image_picker/image_picker.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import 'package:slab_mobile/domain/conversation/conversation_controller.dart';
import 'package:slab_mobile/data/local/drafts_dao.dart';
import 'package:slab_mobile/core/l10n/catalog.dart';
import 'package:slab_mobile/core/l10n/mobile_strings.dart';
import 'package:slab_mobile/core/theme/slab_tokens.g.dart';
import '../../../commands/command_registry.dart';
import '../../../models/model_cubit.dart' show ModelCubit;

/// Max gallery attachments per turn — bounds the JSON-RPC frame size
/// (base64 inflates payloads ~33%).
const _maxAttachments = 4;

class ComposerBar extends StatefulWidget {
  const ComposerBar({
    super.key,
    required this.controller,
    required this.sessionId,
    required this.locale,
    required this.catalog,
    this.onSubmitted,
  });

  final ConversationController controller;
  final String sessionId;
  final String locale;
  final SlabCatalog catalog;

  /// Fired with the raw submitted text on a real send (not control/plan
  /// dispatches) — the chat screen uses it for the first-prompt auto-title.
  final void Function(String text)? onSubmitted;

  @override
  State<ComposerBar> createState() => _ComposerBarState();
}

class _ComposerBarState extends State<ComposerBar> {
  final _composer = TextEditingController();
  final _attachments = <String>[];
  Timer? _draftDebounce;

  /// Turn options riding the composer; null = server default.
  String? _effort;
  String? _permissionMode;

  String get _t => widget.locale;

  @override
  void initState() {
    super.initState();
    _restoreDraft();
    _composer.addListener(_scheduleDraftSave);
  }

  Future<void> _restoreDraft() async {
    if (!GetIt.I.isRegistered<DraftsDao>()) return;
    final draft = await GetIt.I<DraftsDao>().load(widget.sessionId);
    if (!mounted || draft == null) return;
    _composer.value = TextEditingValue(text: draft.content);
    setState(() {
      _effort = draft.effort;
      _permissionMode = draft.permissionMode;
      widget.controller.setPlanMode(draft.planMode);
    });
  }

  void _scheduleDraftSave() {
    _draftDebounce?.cancel();
    _draftDebounce = Timer(const Duration(milliseconds: 600), _saveDraft);
  }

  Future<void> _saveDraft() async {
    if (!GetIt.I.isRegistered<DraftsDao>()) return;
    await GetIt.I<DraftsDao>().save(
      sessionId: widget.sessionId,
      content: _composer.text,
      planMode: widget.controller.state.planMode,
      effort: _effort,
      permissionMode: _permissionMode,
    );
  }

  @override
  void dispose() {
    _draftDebounce?.cancel();
    _composer.removeListener(_scheduleDraftSave);
    _composer.dispose();
    super.dispose();
  }

  // ── send / dispatch ───────────────────────────────────────────────────────

  Future<void> _submit() async {
    final text = _composer.text.trim();
    final hasAttachments = _attachments.isNotEmpty;
    if (text.isEmpty && !hasAttachments) return;

    final dispatch = resolveCommandDispatch(text, widget.controller.state.commands);
    switch (dispatch) {
      case ControlDispatch(:final controlAction):
        _composer.clear();
        switch (controlAction) {
          case 'compact':
            await widget.controller.compactThread();
          case 'fork':
            await widget.controller.forkThread();
        }
      case TogglePlanDispatch():
        _composer.clear();
        widget.controller.setPlanMode(!widget.controller.state.planMode);
      case SendDispatch():
        final modelId = context.read<ModelCubit>().state.selectedId;
        _composer.clear();
        final images = List.of(_attachments);
        _attachments.clear();
        setState(() {});
        await widget.controller.send(
          text: text,
          imageUrls: images,
          effort: _effort,
          permissionMode: _permissionMode,
          modelId: modelId,
        );
        widget.onSubmitted?.call(text);
        if (GetIt.I.isRegistered<DraftsDao>()) {
          await GetIt.I<DraftsDao>().clear(widget.sessionId);
        }
    }
  }

  // ── attachments ───────────────────────────────────────────────────────────

  Future<void> _pickImage() async {
    if (_attachments.length >= _maxAttachments) return;
    final picked = await ImagePicker().pickImage(
      source: ImageSource.gallery,
      maxWidth: 1600,
      maxHeight: 1600,
      imageQuality: 85,
    );
    if (picked == null) return;
    final bytes = await picked.readAsBytes();
    if (!mounted) return;
    // Downscale guard passed; encode as a data URL the harness accepts.
    final dataUrl = 'data:image/jpeg;base64,${base64Encode(bytes)}';
    setState(() => _attachments.add(dataUrl));
  }

  // ── menus ─────────────────────────────────────────────────────────────────

  Future<void> _pickPermissionMode() async {
    final t = widget.catalog.t;
    final modes = [
      ('request_approval', t('pages.assistant.composer.permission.requestApproval')),
      ('approve_for_me', t('pages.assistant.composer.permission.approveForMe')),
      ('full_control', t('pages.assistant.composer.permission.fullControl')),
      ('custom', t('pages.assistant.composer.permission.custom')),
    ];
    final picked = await showModalBottomSheet<String>(
      context: context,
      builder: (context) => SafeArea(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Padding(
              padding: const EdgeInsets.all(12),
              child: Text(t('pages.assistant.composer.permission.title'),
                  style: const TextStyle(fontSize: 13, fontWeight: FontWeight.w600)),
            ),
            for (final (mode, label) in modes)
              TCell(
                title: Text(label),
                arrow: false,
                onTap: () => Navigator.of(context).pop(mode),
              ),
          ],
        ),
      ),
    );
    if (picked == null) return;
    setState(() => _permissionMode = picked);
    _scheduleDraftSave();
  }

  /// Command suggestions for the current `/`-prefixed input.
  List<(String, String)> get _matchingCommands {
    final text = _composer.text;
    if (!text.startsWith('/')) return const [];
    final parsed = parseAssistantCommand(text);
    final prefix = parsed?.name ?? text.substring(1);
    return widget.controller.state.commands
        .where((command) =>
            command.name.startsWith(prefix) || command.aliases.any((alias) => alias.startsWith(prefix)))
        .map((command) => (command.name, command.description))
        .toList(growable: false);
  }

  @override
  Widget build(BuildContext context) {
    final td = context.tTheme;
    final catalog = widget.catalog;
    String t(String key, [Map<String, String> args = const {}]) => mobileT(_t, key, args);

    return ListenableBuilder(
      listenable: Listenable.merge([widget.controller, _composer]),
      builder: (context, _) {
        final state = widget.controller.state;
        final running = state.turnPhase != TurnPhase.idle;
        final commands = _matchingCommands;
        final showCommandMenu = commands.isNotEmpty && _composer.text.startsWith('/');

        return Padding(
          padding: const EdgeInsets.fromLTRB(8, 4, 8, 8),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              if (showCommandMenu)
                Container(
                  constraints: const BoxConstraints(maxHeight: 180),
                  margin: const EdgeInsets.only(bottom: 4),
                  decoration: BoxDecoration(
                    color: td.bgColorContainer,
                    borderRadius: BorderRadius.circular(SlabMetrics.radiusMd),
                    border: Border.all(color: td.componentStrokeColor),
                  ),
                  child: ListView.builder(
                    shrinkWrap: true,
                    itemCount: math.min(commands.length, 6),
                    itemBuilder: (context, index) {
                      final (name, description) = commands[index];
                      return TCell(
                        title: Text('/$name',
                            style: TextStyle(
                                fontSize: SlabMetrics.textCaption,
                                fontFamilyFallback: SlabMetrics.fontMono)),
                        subtitle: description.isEmpty
                            ? null
                            : Text(description,
                                style: TextStyle(
                                    fontSize: SlabMetrics.textMicro,
                                    color: td.textColorPlaceholder)),
                        arrow: false,
                        onTap: () {
                          _composer.value = TextEditingValue(
                            text: '/$name ',
                            selection: const TextSelection.collapsed(offset: 99),
                          );
                        },
                      );
                    },
                  ),
                ),
              if (_attachments.isNotEmpty)
                SizedBox(
                  height: 64,
                  child: ListView(
                    scrollDirection: Axis.horizontal,
                    children: [
                      for (final (index, dataUrl) in _attachments.indexed)
                        Stack(
                          children: [
                            Padding(
                              padding: const EdgeInsets.only(right: 8),
                              child: ClipRRect(
                                borderRadius: BorderRadius.circular(SlabMetrics.radiusSm),
                                child: Image.network(dataUrl,
                                    width: 64, height: 64, fit: BoxFit.cover),
                              ),
                            ),
                            Positioned(
                              right: 0,
                              top: 0,
                              child: GestureDetector(
                                onTap: () => setState(() => _attachments.removeAt(index)),
                                child: Container(
                                  decoration: BoxDecoration(
                                      shape: BoxShape.circle, color: td.bgColorContainer),
                                  child: Icon(TIcons.close,
                                      size: 14, color: td.errorNormalColor),
                                ),
                              ),
                            ),
                          ],
                        ),
                    ],
                  ),
                ),
              Row(
                crossAxisAlignment: CrossAxisAlignment.end,
                children: [
                  // Plus button: image attach today; voice input is a
                  // deferred seam (server transcription is path-based).
                  GestureDetector(
                    onTap: _pickImage,
                    child: Icon(
                      TIcons.add_circle,
                      size: 24,
                      color: _attachments.length >= _maxAttachments ? td.textColorPlaceholder : td.brandNormalColor,
                    ),
                  ),
                  const SizedBox(width: 4),
                  Expanded(
                    child: TTextarea(
                      controller: _composer,
                      minLines: 1,
                      maxLines: 5,
                      hintText: t('mobile.chat.inputHint'),
                      onSubmitted: (_) => _submit(),
                    ),
                  ),
                  const SizedBox(width: 8),
                  running
                      ? TButton(
                          icon: Icon(TIcons.stop_circle),
                          size: TButtonSize.small,
                          colorScheme: TButtonColorScheme.danger,
                          onPressed: widget.controller.interrupt,
                        )
                      : TButton(
                          icon: Icon(TIcons.send),
                          size: TButtonSize.small,
                          colorScheme: TButtonColorScheme.primary,
                          onPressed: _submit,
                        ),
                ],
              ),
              if (!running) _optionsRow(context, catalog),
            ],
          ),
        );
      },
    );
  }

  /// Option chips: plan mode, reasoning effort, permission mode. Values are
  /// composer state, not turn state — they ride the next send.
  Widget _optionsRow(BuildContext context, SlabCatalog catalog) {
    final t = catalog.t;
    // Effort group: the first chip is the "server default" (Auto) option;
    // low/medium/high ride turn/start as `effort`.
    final effortOptions = [
      (null as String?, '${t('pages.assistant.composer.reasoningEffort')}: ${t('mobile.chat.effortAuto')}'),
      ('low', t('pages.assistant.composer.reasoning.low')),
      ('medium', t('pages.assistant.composer.reasoning.medium')),
      ('high', t('pages.assistant.composer.reasoning.high')),
    ];
    return Padding(
      padding: const EdgeInsets.only(top: 4),
      child: Wrap(
        spacing: 6,
        runSpacing: 4,
        children: [
          _chip(
            context: context,
            selected: widget.controller.state.planMode,
            label: widget.controller.state.planMode
                ? '${t('pages.assistant.composer.interaction.plan')} · ${t('pages.assistant.planMode.exit')}'
                : t('pages.assistant.composer.interaction.plan'),
            onTap: () {
              widget.controller.setPlanMode(!widget.controller.state.planMode);
              _scheduleDraftSave();
            },
          ),
          for (final (effort, label) in effortOptions)
            if (effort == null || effort != _effort)
              _chip(
                context: context,
                selected: false,
                label: effort == null && _effort == null ? t('mobile.chat.effortAuto') : label,
                onTap: () {
                  setState(() => _effort = effort);
                  _scheduleDraftSave();
                },
              ),
          if (_effort != null)
            _chip(
              context: context,
              selected: true,
              label: switch (_effort) {
                'low' => t('pages.assistant.composer.reasoning.low'),
                'medium' => t('pages.assistant.composer.reasoning.medium'),
                'high' => t('pages.assistant.composer.reasoning.high'),
                _ => _effort!,
              },
              onTap: () {
                setState(() => _effort = null);
                _scheduleDraftSave();
              },
            ),
          _chip(
            context: context,
            selected: _permissionMode != null,
            label: _permissionMode == null
                ? t('pages.assistant.composer.permission.title')
                : switch (_permissionMode) {
                    'request_approval' => t('pages.assistant.composer.permission.requestApproval'),
                    'approve_for_me' => t('pages.assistant.composer.permission.approveForMe'),
                    'full_control' => t('pages.assistant.composer.permission.fullControl'),
                    'custom' => t('pages.assistant.composer.permission.custom'),
                    _ => t('pages.assistant.composer.permission.title'),
                  },
            onTap: _pickPermissionMode,
          ),
        ],
      ),
    );
  }

  Widget _chip({
    required BuildContext context,
    required bool selected,
    required String label,
    required VoidCallback onTap,
  }) {
    return Theme(
      // Chip styling rides the TTagThemeData extension (TDesign 1.0).
      data: Theme.of(context).copyWith(extensions: [TTagThemeData(isOutline: !selected)]),
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(SlabMetrics.radiusSm),
        child: TTag(
          label,
          size: TTagSize.small,
          colorScheme: selected ? TTagColorScheme.primary : TTagColorScheme.defaultTheme,
        ),
      ),
    );
  }
}
