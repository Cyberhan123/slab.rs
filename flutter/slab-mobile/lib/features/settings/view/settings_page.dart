/// Settings tab: schema-driven section browser over the settings document.
/// Search filters across sections/properties; tapping a section pushes its
/// detail page (field cards + PopScope unsaved-changes guard). Warnings and
/// the non-loopback-without-admin-token alert render at the top. Port of
/// the desktop `settings/index.tsx` layout at mobile scope.
library;

import 'package:flutter/material.dart';
import 'package:flutter_bloc/flutter_bloc.dart';
import 'package:tdesign_flutter/tdesign_flutter.dart';

import 'package:slab_mobile/core/app/connection_cubit.dart';
import 'package:slab_mobile/core/app/locale_cubit.dart';
import 'package:slab_mobile/data/rest/settings_types.dart';
import 'package:slab_mobile/core/l10n/catalog.dart';
import 'package:slab_mobile/core/theme/slab_tokens.g.dart';
import '../autosave/request_body.dart';
import '../settings_cubit.dart';
import 'widgets/field_card.dart';

class SettingsPage extends StatelessWidget {
  const SettingsPage({super.key, this.cubit});

  /// Test seam: a pre-built cubit replaces the page-owned one.
  final SettingsCubit? cubit;

  @override
  Widget build(BuildContext context) {
    final provided = cubit;
    return provided == null
        ? BlocProvider(
            create: (context) => SettingsCubit(client: context.read<ConnectionCubit>().client!)..load(),
            child: const _SettingsView(),
          )
        : BlocProvider.value(value: provided, child: const _SettingsView());
  }
}

class _SettingsView extends StatelessWidget {
  const _SettingsView();

  @override
  Widget build(BuildContext context) {
    final catalog = context.watch<LocaleCubit>().catalog;
    final state = context.watch<SettingsCubit>().state;

    return Scaffold(
      body: Column(
        children: [
          TNavBar(title: catalog.t('layouts.sidebar.items.settings'), useDefaultBack: false),
          Padding(
            padding: const EdgeInsets.fromLTRB(12, 4, 12, 4),
            child: TInput(
              controller: TextEditingController(text: state.search),
              hintText: catalog.t('pages.settings.search.placeholder'),
              onChanged: (query) => context.read<SettingsCubit>().setSearch(query),
            ),
          ),
          Expanded(child: _body(context, catalog, state)),
        ],
      ),
    );
  }

  Widget _body(BuildContext context, SlabCatalog catalog, SettingsState state) {
    final t = catalog.t;
    if (state.loading) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const TLoading(size: TLoadingSize.medium, icon: TLoadingIcon.circle),
            const SizedBox(height: 12),
            Text(t('pages.settings.page.loadingTitle')),
            Text(t('pages.settings.page.loadingDescription'),
                style: const TextStyle(fontSize: 11)),
          ],
        ),
      );
    }
    if (state.error != null) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(t('pages.settings.page.failedLoadTitle')),
            const SizedBox(height: 4),
            Text(state.error!, style: const TextStyle(fontSize: 11)),
            const SizedBox(height: 8),
            TButton(
              variant: TButtonVariant.outline,
              size: TButtonSize.small,
              onPressed: () => context.read<SettingsCubit>().load(),
              child: Text(t('common.actions.tryAgain')),
            ),
          ],
        ),
      );
    }
    final document = state.document;
    if (document == null) return const SizedBox.shrink();

    return ListView(
      padding: const EdgeInsets.only(bottom: 24),
      children: [
        for (final warning in document.warnings)
          _alert(context, title: t('pages.settings.page.warningsTitle'), body: warning),
        if (_needsAdminTokenWarning(document))
          _alert(
            context,
            title: t('pages.settings.page.adminTokenWarningTitle'),
            body: t('pages.settings.page.adminTokenWarningDescription'),
            danger: true,
          ),
        _sectionsOrSearch(context, catalog, document, state.search),
      ],
    );
  }

  Widget _sectionsOrSearch(
    BuildContext context,
    SlabCatalog catalog,
    SettingsDocumentView document,
    String search,
  ) {
    final t = catalog.t;
    if (search.trim().isEmpty) {
      if (document.sections.isEmpty) {
        return Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            children: [
              TEmpty(emptyText: t('pages.settings.page.noSettingsTitle')),
              Text(t('pages.settings.page.noSettingsDescription'), style: const TextStyle(fontSize: 11)),
            ],
          ),
        );
      }
      return Column(
        children: [
          for (final section in document.sections)
            TCell(
              title: Text(section.title),
              subtitle: Text(_sectionSummary(context, catalog, section)),
              arrow: true,
              onTap: () => _openSection(context, catalog, section),
            ),
        ],
      );
    }

    // Search mode: flat matching-property list.
    final matches = <SettingPropertyView>[];
    for (final section in document.sections) {
      if (!searchMatchesSection(section, search)) continue;
      for (final subsection in section.subsections) {
        for (final property in subsection.properties) {
          if (searchMatchesProperty(property, search)) matches.add(property);
        }
      }
    }
    if (matches.isEmpty) {
      return Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          children: [
            TEmpty(emptyText: t('pages.settings.search.noResultsTitle')),
            Text(
              t('pages.settings.search.noResultsDescription', {'query': search.trim()}),
              style: const TextStyle(fontSize: 11),
            ),
          ],
        ),
      );
    }
    return Column(
      children: [for (final property in matches) SettingFieldCard(property: property, catalog: catalog)],
    );
  }

  String _sectionSummary(BuildContext context, SlabCatalog catalog, SettingsSectionView section) {
    var count = 0;
    for (final subsection in section.subsections) {
      count += subsection.properties.length;
    }
    final dirty = _dirtyInSection(context, section);
    final t = catalog.t;
    final base = t('pages.settings.page.settingsCount_other', {'count': '$count'});
    if (dirty > 0) return '$base · ${t('pages.settings.page.pending_other', {'count': '$dirty'})}';
    return base;
  }

  int _dirtyInSection(BuildContext context, SettingsSectionView section) {
    final statuses = context.watch<SettingsCubit>().state.fieldStatus;
    var dirty = 0;
    for (final subsection in section.subsections) {
      for (final property in subsection.properties) {
        final status = statuses[property.pmid];
        if (status == FieldStatus.dirty || status == FieldStatus.saving || status == FieldStatus.error) dirty += 1;
      }
    }
    return dirty;
  }

  void _openSection(BuildContext context, SlabCatalog catalog, SettingsSectionView section) {
    Navigator.of(context).push(
      MaterialPageRoute<void>(
        builder: (pageContext) => BlocProvider.value(
          value: context.read<SettingsCubit>(),
          child: _SectionDetailPage(section: section, catalog: catalog),
        ),
      ),
    );
  }

  /// Non-loopback bind without an admin token = open server: surface the
  /// destructive-config warning (desktop parity).
  bool _needsAdminTokenWarning(SettingsDocumentView document) {
    String? address;
    String? adminToken;
    for (final section in document.sections) {
      for (final subsection in section.subsections) {
        for (final property in subsection.properties) {
          if (property.pmid == 'server.address') address = property.effectiveValue?.toString();
          if (property.pmid == 'server.admin.token') adminToken = property.effectiveValue?.toString();
        }
      }
    }
    if (address == null) return false;
    final loopback = address == '127.0.0.1' || address == 'localhost' || address == '::1';
    return !loopback && (adminToken == null || adminToken.isEmpty);
  }

  Widget _alert(BuildContext context, {required String title, required String body, bool danger = false}) {
    final td = context.tTheme;
    final color = danger ? td.errorNormalColor : td.warningNormalColor;
    return Container(
      margin: const EdgeInsets.fromLTRB(12, 6, 12, 0),
      padding: const EdgeInsets.all(10),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.08),
        borderRadius: BorderRadius.circular(SlabMetrics.radiusMd),
        border: Border.all(color: color.withValues(alpha: 0.5)),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(TIcons.error_circle, size: 14, color: color),
          const SizedBox(width: 6),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(title, style: TextStyle(fontSize: 12, fontWeight: FontWeight.w600, color: color)),
                const SizedBox(height: 2),
                Text(body, style: TextStyle(fontSize: 11, height: 1.4)),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

/// Section detail: one card per property, back-guarded while unsaved
/// changes are in flight.
class _SectionDetailPage extends StatelessWidget {
  const _SectionDetailPage({required this.section, required this.catalog});

  final SettingsSectionView section;
  final SlabCatalog catalog;

  Future<void> _confirmLeave(BuildContext context) async {
    final t = catalog.t;
    final state = context.read<SettingsCubit>().state;
    final pending = state.dirtyCount + state.savingCount + state.errorCount;
    final confirmed = await TDialog.show<bool>(
      context,
      dialog: TDialog(
        title: Text(t('pages.settings.guard.title')),
        content: Text(t('pages.settings.guard.description', {'count': '$pending'})),
        actions: [
          TDialogAction(child: Text(t('pages.settings.guard.stay')), result: false),
          TDialogAction(
            child: Text(t('pages.settings.guard.leave')),
            result: true,
            role: TDialogActionRole.destructive,
          ),
        ],
      ),
    );
    if (confirmed == true && context.mounted) {
      Navigator.of(context).pop(true);
    }
  }

  @override
  Widget build(BuildContext context) {
    return PopScope(
      canPop: !context.watch<SettingsCubit>().state.hasUnsavedChanges,
      onPopInvokedWithResult: (didPop, _) {
        if (!didPop) _confirmLeave(context);
      },
      child: Scaffold(
        body: Column(
          children: [
            TNavBar(
              title: section.title,
              useDefaultBack: true,
              onBack: () => Navigator.of(context).maybePop(),
            ),
            Expanded(
              child: ListView(
                padding: const EdgeInsets.only(top: 4, bottom: 24),
                children: [
                  for (final subsection in section.subsections) ...[
                    Padding(
                      padding: const EdgeInsets.fromLTRB(16, 10, 16, 2),
                      child: Text(
                        subsection.title,
                        style: TextStyle(
                          fontSize: SlabMetrics.textCaption,
                          fontWeight: FontWeight.w700,
                          color: context.tTheme.textColorSecondary,
                        ),
                      ),
                    ),
                    for (final property in subsection.properties)
                      SettingFieldCard(property: property, catalog: catalog),
                  ],
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}
