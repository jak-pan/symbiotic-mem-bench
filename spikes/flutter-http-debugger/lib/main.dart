import 'dart:async';
import 'dart:math' as math;
import 'package:flutter/material.dart';
// ScrollCacheExtent (Flutter 3.44+ replacement for the deprecated int cacheExtent)
import 'package:flutter/rendering.dart';

import 'src/api/debugger.dart';

const bg = Color(0xFF07080A);
const panel = Color(0xFF101319);
const elev = Color(0xFF171B23);
const rowBg = Color(0xFF0C0F14);
const selectedBg = Color(0xFF201A12);
const border = Color(0xFF262B36);
const borderBright = Color(0xFF3A414F);
const amber = Color(0xFFFFA524);
const amberDim = Color(0x66FFA524);
const text = Color(0xFFCCD2DB);
// Sidebar / metadata text colors. The previous values (#828B98 / #586170)
// sat near the WCAG AA threshold against the #07080A background and were
// essentially unreadable in the registry sidebar where many labels are
// shown at once. Bumped to lighter greys that clear AA comfortably on both
// the root `bg` and the sidebar `panel` surfaces:
//   - dim  (#9AA3B2, ~65% L) → 7.6:1 on bg, 6.4:1 on panel  (AAA body text)
//   - faint(#737D8B, ~50% L) → 5.5:1 on bg, 4.7:1 on panel  (AA body text)
// Hierarchy preserved: dim is still visibly stronger than faint.
const dim = Color(0xFFA0A8B8);
const faint = Color(0xFF7A8294);
const green = Color(0xFF2FCF7A);
const red = Color(0xFFFF5347);
const cyan = Color(0xFF35D0FF);
const violet = Color(0xFFA78BFA);

bool apiReady = false;
String? apiBootstrapError;
final apiStatusNotifier = ValueNotifier<String>('backend API starting…');

void main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await _bootstrapApi();
  runApp(const DebuggerApp());
}

Future<void> _bootstrapApi() async {
  try {
    final health = await bridgeHealth().timeout(
      const Duration(seconds: 2),
      onTimeout: () => throw TimeoutException('API health timed out'),
    );
    apiReady = health.ok;
    apiStatusNotifier.value = health.ok
        ? 'api ok · v${health.version} · ${health.apiBaseUrl}'
        : 'api unhealthy · v${health.version}';
  } catch (error, stack) {
    apiReady = false;
    apiBootstrapError = error.toString();
    apiStatusNotifier.value = 'api unavailable: $error';
    debugPrint('api bootstrap failed; showing app anyway: $error\n$stack');
  }
}

class DebuggerApp extends StatelessWidget {
  const DebuggerApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      debugShowCheckedModeBanner: false,
      title: 'MEMBENCH Debugger HTTP',
      theme: ThemeData(
        brightness: Brightness.dark,
        scaffoldBackgroundColor: bg,
        fontFamily: 'Avenir Next',
        colorScheme: const ColorScheme.dark(
          primary: amber,
          surface: panel,
          onSurface: text,
        ),
        textSelectionTheme: const TextSelectionThemeData(cursorColor: amber),
        useMaterial3: true,
      ),
      home: const DebuggerPage(),
    );
  }
}

class DebuggerPage extends StatefulWidget {
  const DebuggerPage({super.key});

  @override
  State<DebuggerPage> createState() => _DebuggerPageState();
}

class _DebuggerPageState extends State<DebuggerPage> {
  RegistryView? registry;
  String? selectedId;
  String activeTab = 'overview';
  String sort = 'score';
  bool showBenchmarks = true;
  bool showTuning = true;
  bool showTrials = true;
  bool showRuns = true;
  bool showRecords = true;
  bool showStale = false;
  bool loadingRegistry = false;
  String? error;
  String apiStatus = apiStatusNotifier.value;
  late final VoidCallback apiListener;

  RegistryRunNode? get selectedNode {
    final view = registry;
    if (view == null || selectedId == null) return null;
    for (final group in view.groups) {
      for (final run in group.runs) {
        if (run.runId == selectedId) return run;
      }
    }
    return null;
  }

  @override
  void initState() {
    super.initState();
    final query = Uri.base.queryParameters;
    final requestedRun = query['run']?.trim();
    final requestedTab = query['tab']?.trim().toLowerCase();
    if (requestedRun != null && requestedRun.isNotEmpty) {
      selectedId = requestedRun;
    }
    if (requestedTab != null &&
        const {
          'overview',
          'questions',
          'traces',
          'live',
        }.contains(requestedTab)) {
      activeTab = requestedTab;
    }
    apiListener = () {
      if (!mounted) return;
      setState(() => apiStatus = apiStatusNotifier.value);
      if (apiReady && registry == null && !loadingRegistry) {
        unawaited(_loadRegistry());
      }
    };
    apiStatusNotifier.addListener(apiListener);
    if (apiReady) unawaited(_loadRegistry());
  }

  @override
  void dispose() {
    apiStatusNotifier.removeListener(apiListener);
    super.dispose();
  }

  Future<void> _loadRegistry() async {
    if (!apiReady) return;
    setState(() {
      loadingRegistry = true;
      error = null;
    });
    try {
      final next = await loadRegistry(
        sort: sort,
        showBenchmarks: showBenchmarks,
        showTuning: showTuning,
        showTrials: showTrials,
        showRuns: showRuns,
        showRecords: showRecords,
        showStale: showStale,
      );
      if (!mounted) return;
      setState(() {
        registry = next;
        if ((selectedId == null || selectedId!.isEmpty) &&
            next.defaultRunId.isNotEmpty) {
          selectedId = next.defaultRunId;
        }
      });
    } catch (err) {
      if (mounted) setState(() => error = err.toString());
    } finally {
      if (mounted) setState(() => loadingRegistry = false);
    }
  }

  void _selectRun(String runId, {bool live = false}) {
    setState(() {
      selectedId = runId;
      if (live) activeTab = 'live';
    });
  }

  void _setSort(String value) {
    setState(() => sort = value);
    unawaited(_loadRegistry());
  }

  void _toggleKind(String kind) {
    final nb = kind == 'benchmarks' ? !showBenchmarks : showBenchmarks;
    final nt = kind == 'tuning' ? !showTuning : showTuning;
    final nr = kind == 'trials' ? !showTrials : showTrials;
    if (!nb && !nt && !nr) return;
    setState(() {
      showBenchmarks = nb;
      showTuning = nt;
      showTrials = nr;
    });
    unawaited(_loadRegistry());
  }

  void _toggleSource(String source) {
    final nr = source == 'runs' ? !showRuns : showRuns;
    final nc = source == 'records' ? !showRecords : showRecords;
    if (!nr && !nc) return;
    setState(() {
      showRuns = nr;
      showRecords = nc;
    });
    unawaited(_loadRegistry());
  }

  @override
  Widget build(BuildContext context) {
    final runId = selectedId;
    final node = selectedNode;
    return Scaffold(
      body: SafeArea(
        child: Column(
          children: [
            Expanded(
              child: Row(
                children: [
                  SizedBox(
                    width: 264,
                    child: RegistryTree(
                      view: registry,
                      selectedId: selectedId,
                      sort: sort,
                      loading: loadingRegistry,
                      showBenchmarks: showBenchmarks,
                      showTuning: showTuning,
                      showTrials: showTrials,
                      showRuns: showRuns,
                      showRecords: showRecords,
                      showStale: showStale,
                      onRefresh: _loadRegistry,
                      onSort: _setSort,
                      onKind: _toggleKind,
                      onSource: _toggleSource,
                      onStale: () {
                        setState(() => showStale = !showStale);
                        unawaited(_loadRegistry());
                      },
                      onSelect: _selectRun,
                    ),
                  ),
                  const VerticalDivider(width: 1, color: border),
                  Expanded(
                    child: Column(
                      children: [
                        _TabBar(
                          selectedId: runId,
                          node: node,
                          active: activeTab,
                          onTab: (tab) => setState(() => activeTab = tab),
                        ),
                        Expanded(
                          child: error != null
                              ? _ErrorState(
                                  error: error!,
                                  onRetry: _loadRegistry,
                                )
                              : runId == null || runId.isEmpty
                              ? const _EmptyState(
                                  'SELECT A RUN FROM THE REGISTRY',
                                )
                              : _AnimatedRunBody(
                                  runId: runId,
                                  tab: activeTab,
                                  child: _TabView(tab: activeTab, runId: runId),
                                ),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
            _StatusBar(
              status: apiStatus,
              registry: registry,
              selectedId: selectedId,
            ),
          ],
        ),
      ),
    );
  }
}

class RegistryTree extends StatefulWidget {
  const RegistryTree({
    super.key,
    required this.view,
    required this.selectedId,
    required this.sort,
    required this.loading,
    required this.showBenchmarks,
    required this.showTuning,
    required this.showTrials,
    required this.showRuns,
    required this.showRecords,
    required this.showStale,
    required this.onRefresh,
    required this.onSort,
    required this.onKind,
    required this.onSource,
    required this.onStale,
    required this.onSelect,
  });

  final RegistryView? view;
  final String? selectedId;
  final String sort;
  final bool loading;
  final bool showBenchmarks;
  final bool showTuning;
  final bool showTrials;
  final bool showRuns;
  final bool showRecords;
  final bool showStale;
  final VoidCallback onRefresh;
  final ValueChanged<String> onSort;
  final ValueChanged<String> onKind;
  final ValueChanged<String> onSource;
  final VoidCallback onStale;
  final void Function(String, {bool live}) onSelect;

  @override
  State<RegistryTree> createState() => _RegistryTreeState();
}

class _RegistryTreeState extends State<RegistryTree> {
  final ScrollController _registryScroll = ScrollController();

  @override
  void dispose() {
    _registryScroll.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final data = widget.view;
    final selectedId = widget.selectedId;
    final sort = widget.sort;
    final loading = widget.loading;
    final showBenchmarks = widget.showBenchmarks;
    final showTuning = widget.showTuning;
    final showTrials = widget.showTrials;
    final showRuns = widget.showRuns;
    final showRecords = widget.showRecords;
    final showStale = widget.showStale;
    final onRefresh = widget.onRefresh;
    final onSort = widget.onSort;
    final onKind = widget.onKind;
    final onSource = widget.onSource;
    final onStale = widget.onStale;
    final onSelect = widget.onSelect;
    return Container(
      color: panel,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          if (data != null && data.pendingTotal > 0) ...[
            _TreeHeader(
              label: 'IN FLIGHT',
              trailing: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  if (data.runningCount > 0)
                    Text('● ${data.runningCount} live', style: mini(green)),
                  if (data.warningCount > 0)
                    Padding(
                      padding: const EdgeInsets.only(left: 6),
                      child: Text(
                        '◐ ${data.warningCount} idle',
                        style: mini(amber),
                      ),
                    ),
                ],
              ),
            ),
            for (final p in data.pending)
              _PendingNode(
                row: p,
                selected: p.runId == selectedId,
                onTap: () => onSelect(p.runId, live: true),
              ),
            if (data.staleCount > 0)
              Padding(
                padding: const EdgeInsets.symmetric(
                  horizontal: 10,
                  vertical: 4,
                ),
                child: _FilterChip(
                  label: showStale
                      ? 'hide stale (${data.staleCount})'
                      : 'show stale (${data.staleCount})',
                  active: showStale,
                  onTap: onStale,
                ),
              ),
            const Divider(height: 1, color: border),
          ],
          _TreeHeader(
            label: 'REGISTRY',
            trailing: DropdownButton<String>(
              value: sort,
              dropdownColor: elev,
              isDense: true,
              underline: const SizedBox.shrink(),
              style: mini(dim),
              items: const [
                DropdownMenuItem(value: 'score', child: Text('score')),
                DropdownMenuItem(value: 'newest', child: Text('newest')),
                DropdownMenuItem(value: 'oldest', child: Text('oldest')),
              ],
              onChanged: (v) {
                if (v != null) onSort(v);
              },
            ),
          ),
          _FilterBar(
            children: [
              _FilterChip(
                label: 'bench',
                active: showBenchmarks,
                onTap: () => onKind('benchmarks'),
              ),
              _FilterChip(
                label: 'tuning',
                active: showTuning,
                onTap: () => onKind('tuning'),
              ),
              _FilterChip(
                label: 'trials',
                active: showTrials,
                onTap: () => onKind('trials'),
              ),
            ],
          ),
          _FilterBar(
            children: [
              _FilterChip(
                label: 'runs',
                active: showRuns,
                onTap: () => onSource('runs'),
              ),
              _FilterChip(
                label: 'records',
                active: showRecords,
                onTap: () => onSource('records'),
              ),
              const Spacer(),
              if (data != null && data.hiddenRegistryCount > 0)
                Text('${data.hiddenRegistryCount} hidden', style: mini(faint)),
            ],
          ),
          const Divider(height: 1, color: border),
          Expanded(
            child: data == null
                ? Center(
                    child: Text(
                      loading ? 'SCANNING REGISTRY…' : 'NO REGISTRY',
                      style: body(faint),
                    ),
                  )
                : RefreshIndicator(
                    onRefresh: () async => onRefresh(),
                    // Use SingleChildScrollView + Column, not ListView, for
                    // the grouped registry. The groups have variable heights;
                    // ListView's lazy sliver estimate makes the scrollbar thumb
                    // jump/change height as real extents are discovered.
                    child: Scrollbar(
                      controller: _registryScroll,
                      notificationPredicate: (notification) =>
                          notification.depth == 0,
                      child: SingleChildScrollView(
                        controller: _registryScroll,
                        primary: false,
                        physics: const AlwaysScrollableScrollPhysics(
                          parent: ClampingScrollPhysics(
                            parent: BouncingScrollPhysics(),
                          ),
                        ),
                        keyboardDismissBehavior:
                            ScrollViewKeyboardDismissBehavior.onDrag,
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          children: [
                            if (data.groups.isEmpty)
                              const Padding(
                                padding: EdgeInsets.all(14),
                                child: Text(
                                  'No runs match the active toggles.',
                                  style: TextStyle(color: faint, fontSize: 11),
                                ),
                              ),
                            for (final group in data.groups)
                              _RegistryGroupWidget(
                                group: group,
                                selectedId: selectedId,
                                onSelect: onSelect,
                              ),
                          ],
                        ),
                      ),
                    ),
                  ),
          ),
        ],
      ),
    );
  }
}

class _RegistryGroupWidget extends StatelessWidget {
  const _RegistryGroupWidget({
    required this.group,
    required this.selectedId,
    required this.onSelect,
  });
  final RegistryGroup group;
  final String? selectedId;
  final void Function(String, {bool live}) onSelect;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 6),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Container(
            padding: const EdgeInsets.fromLTRB(10, 6, 10, 5),
            decoration: const BoxDecoration(
              border: Border(bottom: BorderSide(color: border)),
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(group.label.toUpperCase(), style: labelStyle(amber)),
                Text(
                  group.sublabel,
                  overflow: TextOverflow.ellipsis,
                  style: mini(faint),
                ),
              ],
            ),
          ),
          for (final run in group.runs)
            _RunNode(
              row: run,
              selected: run.runId == selectedId,
              onTap: () => onSelect(run.runId),
            ),
        ],
      ),
    );
  }
}

/// Pending-run row with hover parity to `_RunNode`: background lifts to
/// `elev`, accent border slides in on hover, text brightens.
class _PendingNode extends StatefulWidget {
  const _PendingNode({
    required this.row,
    required this.selected,
    required this.onTap,
  });
  final PendingNode row;
  final bool selected;
  final VoidCallback onTap;

  @override
  State<_PendingNode> createState() => _PendingNodeState();
}

class _PendingNodeState extends State<_PendingNode> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final selected = widget.selected;
    final accent = _hovered || selected;
    return MouseRegion(
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: InkWell(
        onTap: widget.onTap,
        hoverColor: elev,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 140),
          curve: Curves.easeOutCubic,
          decoration: BoxDecoration(
            color: selected
                ? selectedBg
                : (_hovered ? elev : Colors.transparent),
            border: Border(
              left: BorderSide(
                color: accent ? amber : Colors.transparent,
                width: 2,
              ),
            ),
          ),
          padding: const EdgeInsets.fromLTRB(14, 5, 10, 5),
          child: Row(
            children: [
              _Dot(color: statusColor(widget.row.status)),
              const SizedBox(width: 7),
              Expanded(
                child: AnimatedDefaultTextStyle(
                  duration: const Duration(milliseconds: 140),
                  style: body(selected || _hovered ? text : dim),
                  child: Text(
                    widget.row.runName,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              ),
              Text(widget.row.progress, style: mono(cyan, 10)),
            ],
          ),
        ),
      ),
    );
  }
}

/// Registry row that lifts on hover (background shift + accent border) and
/// slides the accent border in/out. Hover state is local; the parent
/// `selected` prop drives the persistent amber-bordered state.
class _RunNode extends StatefulWidget {
  const _RunNode({
    required this.row,
    required this.selected,
    required this.onTap,
  });
  final RegistryRunNode row;
  final bool selected;
  final VoidCallback onTap;

  @override
  State<_RunNode> createState() => _RunNodeState();
}

class _RunNodeState extends State<_RunNode> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final selected = widget.selected;
    final accent = _hovered || selected;
    return MouseRegion(
      onEnter: (_) => setState(() => _hovered = true),
      onExit: (_) => setState(() => _hovered = false),
      child: InkWell(
        onTap: widget.onTap,
        hoverColor: elev, // Material splash on hover for unselected rows
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 140),
          curve: Curves.easeOutCubic,
          decoration: BoxDecoration(
            color: selected
                ? selectedBg
                : (_hovered ? elev : Colors.transparent),
            border: Border(
              left: BorderSide(
                color: accent ? amber : Colors.transparent,
                width: 2,
              ),
            ),
          ),
          padding: const EdgeInsets.fromLTRB(12, 5, 10, 5),
          child: Row(
            children: [
              _Dot(color: dotColor(widget.row.dotKind)),
              const SizedBox(width: 7),
              Expanded(
                child: AnimatedDefaultTextStyle(
                  duration: const Duration(milliseconds: 140),
                  style: body(selected || _hovered ? text : dim),
                  child: RichText(
                    overflow: TextOverflow.ellipsis,
                    text: TextSpan(
                      style: body(selected || _hovered ? text : dim),
                      children: [
                        TextSpan(text: widget.row.name),
                        const TextSpan(text: '  '),
                        TextSpan(
                          text: widget.row.sourceBadge,
                          style: mini(
                            widget.row.sourceBadge == 'META' ? cyan : faint,
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ),
              Text(widget.row.meta, style: mono(faint, 10.5)),
            ],
          ),
        ),
      ),
    );
  }
}

class _TabBar extends StatelessWidget {
  const _TabBar({
    required this.selectedId,
    required this.node,
    required this.active,
    required this.onTab,
  });
  final String? selectedId;
  final RegistryRunNode? node;
  final String active;
  final ValueChanged<String> onTab;

  @override
  Widget build(BuildContext context) {
    const tabs = [
      ('overview', 'OVERVIEW'),
      ('questions', 'QUESTIONS'),
      ('traces', 'TRACES'),
      ('live', 'LIVE'),
    ];
    return Container(
      height: 38,
      decoration: const BoxDecoration(
        color: panel,
        border: Border(bottom: BorderSide(color: borderBright)),
      ),
      child: Row(
        children: [
          const SizedBox(width: 12),
          if (node != null)
            KindChip(
              label: node!.runKind,
              color: node!.nativeStateAvailable ? green : cyan,
            ),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              selectedId ?? 'no run selected',
              overflow: TextOverflow.ellipsis,
              style: mini(dim),
            ),
          ),
          for (final t in tabs)
            InkWell(
              onTap: () => onTab(t.$1),
              hoverColor: elev, // Tier 1: row hover lift
              child: Container(
                height: 38,
                padding: const EdgeInsets.symmetric(horizontal: 13),
                alignment: Alignment.center,
                decoration: const BoxDecoration(
                  border: Border(left: BorderSide(color: border)),
                ),
                // Animated underline + color crossfade so the active tab
                // highlight slides between tabs instead of blinking.
                child: DefaultTextStyle(
                  style: labelStyle(
                    active == t.$1 ? (t.$1 == 'live' ? green : amber) : faint,
                  ),
                  child: AnimatedContainer(
                    duration: const Duration(milliseconds: 220),
                    curve: Curves.easeOutCubic,
                    decoration: BoxDecoration(
                      border: Border(
                        bottom: BorderSide(
                          color: active == t.$1
                              ? (t.$1 == 'live' ? green : amber)
                              : Colors.transparent,
                          width: 2,
                        ),
                      ),
                    ),
                    child: Center(child: Text(t.$2)),
                  ),
                ),
              ),
            ),
        ],
      ),
    );
  }
}

class _TabView extends StatefulWidget {
  const _TabView({required this.tab, required this.runId});
  final String tab;
  final String runId;

  @override
  State<_TabView> createState() => _TabViewState();
}

class _TabViewState extends State<_TabView> {
  late final Set<String> _visited = {widget.tab};

  @override
  void didUpdateWidget(covariant _TabView oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.runId != widget.runId) {
      // New run: drop hidden heavy tab subtrees for the previous run.
      _visited
        ..clear()
        ..add(widget.tab);
    } else {
      _visited.add(widget.tab);
    }
  }

  Widget _slot(String tab, Widget child) {
    if (!_visited.contains(tab)) return const SizedBox.shrink();
    return TickerMode(enabled: widget.tab == tab, child: child);
  }

  @override
  Widget build(BuildContext context) {
    final idx = switch (widget.tab) {
      'questions' => 1,
      'traces' => 2,
      'live' => 3,
      _ => 0,
    };
    return IndexedStack(
      index: idx,
      children: [
        _slot('overview', OverviewScreen(runId: widget.runId)),
        _slot('questions', QuestionsScreen(runId: widget.runId)),
        _slot('traces', TracesScreen(runId: widget.runId)),
        _slot(
          'live',
          LiveScreen(runId: widget.runId, active: widget.tab == 'live'),
        ),
      ],
    );
  }
}

class OverviewScreen extends StatefulWidget {
  const OverviewScreen({super.key, required this.runId});
  final String runId;

  @override
  State<OverviewScreen> createState() => _OverviewScreenState();
}

class _OverviewScreenState extends State<OverviewScreen> {
  late Future<OverviewView> future = loadOverview(id: widget.runId);

  @override
  void didUpdateWidget(covariant OverviewScreen oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.runId != widget.runId) {
      setState(() => future = loadOverview(id: widget.runId));
    }
  }

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<OverviewView>(
      future: future,
      builder: (context, snap) {
        if (snap.connectionState != ConnectionState.done) {
          return const _EmptyState('LOADING RUN…');
        }
        if (snap.hasError) {
          return _ErrorState(
            error: '${snap.error}',
            onRetry: () =>
                setState(() => future = loadOverview(id: widget.runId)),
          );
        }
        final data = snap.requireData;
        return SingleChildScrollView(
          padding: const EdgeInsets.all(10),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Padding(
                padding: const EdgeInsets.only(bottom: 10),
                child: Text(
                  data.runName,
                  style: mono(amber, 18).copyWith(fontWeight: FontWeight.w800),
                ),
              ),
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Expanded(
                    child: SizedBox(
                      height: 220,
                      child: Panel(
                        title: 'Score',
                        tag: data.runKind,
                        child: Row(
                          children: [
                            RingGauge(
                              value: data.accuracyValue,
                              label: data.accuracyLabel,
                            ),
                            const SizedBox(width: 18),
                            Expanded(
                              child: Wrap(
                                runSpacing: 1,
                                spacing: 1,
                                children: [
                                  for (final row in data.tiles)
                                    _ScoreTile(row: row),
                                ],
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                  ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: SizedBox(
                      height: 220,
                      child: Panel(
                        title: 'Cohort & Models',
                        child: KvTable(rows: data.cohort),
                      ),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 10),
              LayoutBuilder(
                builder: (context, c) {
                  final w = (c.maxWidth - 20) / 3;
                  return Wrap(
                    spacing: 10,
                    runSpacing: 10,
                    children: [
                      SizedBox(
                        width: w,
                        height: 300,
                        child: Panel(
                          title: 'Run Parameters',
                          tag: '${data.params.length} fields',
                          scroll: true,
                          child: KvTable(rows: data.params, monoValues: true),
                        ),
                      ),
                      SizedBox(
                        width: w,
                        height: 300,
                        child: Panel(
                          title: 'Artifacts',
                          child: _Artifacts(rows: data.artifacts),
                        ),
                      ),
                      SizedBox(
                        width: w,
                        height: 300,
                        child: Panel(
                          title: 'Model Calls',
                          tag: '${data.modelRows.length} rows',
                          flush: true,
                          child: _ModelTable(rows: data.modelRows),
                        ),
                      ),
                    ],
                  );
                },
              ),
            ],
          ),
        );
      },
    );
  }
}

class TracesScreen extends StatefulWidget {
  const TracesScreen({super.key, required this.runId});
  final String runId;

  @override
  State<TracesScreen> createState() => _TracesScreenState();
}

class _TracesScreenState extends State<TracesScreen> {
  bool showQueues = false;
  String queueFilter = '';
  Future<TracesView>? future;

  Future<void> _load() async {
    // The bridge bootstrap blocks runApp() in main(), so __state is
    // already set by the time this runs. No per-call wait needed.
    final next = loadTraces(
      id: widget.runId,
      queueFilter: queueFilter,
      showQueueSummary: showQueues,
    );
    if (!mounted) return;
    setState(() => future = next);
  }

  @override
  void initState() {
    super.initState();
    unawaited(_load());
  }

  void reload() => unawaited(_load());

  @override
  void didUpdateWidget(covariant TracesScreen oldWidget) {
    super.didUpdateWidget(oldWidget);
    // IndexedStack preserves State across tab switches AND across run
    // selections in the registry. Without this, picking a different run
    // while on the Traces tab would leave the old run's data on screen.
    if (oldWidget.runId != widget.runId) reload();
  }

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<TracesView>(
      future: future,
      builder: (context, snap) {
        if (snap.connectionState != ConnectionState.done) {
          return const _EmptyState('LOADING TRACES…');
        }
        if (snap.hasError) {
          return _ErrorState(error: '${snap.error}', onRetry: reload);
        }
        final data = snap.requireData;
        return SingleChildScrollView(
          padding: const EdgeInsets.all(10),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              if (data.noneMessage.isNotEmpty) _Notice(data.noneMessage),
              if (data.memoryRows.isNotEmpty)
                Panel(
                  title: 'Memory Work Timing',
                  tag: 'batch/window cadence',
                  flush: true,
                  child: _MemoryTable(rows: data.memoryRows),
                ),
              if (data.bottlenecks.isNotEmpty) ...[
                const SizedBox(height: 10),
                Panel(
                  title: 'Bottleneck Overview',
                  tag: 'p98 slow paths',
                  child: _Bottlenecks(rows: data.bottlenecks),
                ),
              ],
              const SizedBox(height: 10),
              Panel(
                title: 'Provider Queue Summary',
                tag: showQueues
                    ? '${data.queueRows.length} queues'
                    : 'collapsed',
                flush: true,
                action: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    if (showQueues)
                      SizedBox(
                        width: 180,
                        height: 25,
                        child: TextField(
                          style: body(text),
                          decoration: fieldDecoration('filter queue…'),
                          onSubmitted: (v) {
                            queueFilter = v;
                            reload();
                          },
                        ),
                      ),
                    const SizedBox(width: 8),
                    OutlinedButton(
                      onPressed: () {
                        showQueues = !showQueues;
                        reload();
                      },
                      child: Text(showQueues ? 'HIDE' : 'SHOW'),
                    ),
                  ],
                ),
                child: showQueues
                    ? _QueueSummaryTable(rows: data.queueRows)
                    : const _EmptyState('QUEUE SUMMARY COLLAPSED'),
              ),
              if (data.workflowRows.isNotEmpty) ...[
                const SizedBox(height: 10),
                Panel(
                  title: 'Workflow Queue',
                  child: KvTable(rows: data.workflowRows, monoValues: true),
                ),
              ],
              if (data.traceRows.isNotEmpty) ...[
                const SizedBox(height: 10),
                Panel(
                  title: 'Unified Trace Log',
                  flush: true,
                  child: _TraceRows(rows: data.traceRows),
                ),
              ],
              // Always render a placeholder so the panel is findable. If
              // traceRows is empty (some runs don't emit trace events), the
              // header still shows "0 events" and the body explains why.
              if (data.traceRows.isEmpty) ...[
                const SizedBox(height: 10),
                Panel(
                  title: 'Unified Trace Log',
                  tag: '0 events',
                  flush: true,
                  child: const _EmptyState(
                    'NO TRACE EVENTS — this run did not emit a per-event trace stream '
                    '(check memory_traces.jsonl or trace_waterfall in the API response).',
                  ),
                ),
              ],
            ],
          ),
        );
      },
    );
  }
}

class QuestionsScreen extends StatefulWidget {
  const QuestionsScreen({super.key, required this.runId});
  final String runId;

  @override
  State<QuestionsScreen> createState() => _QuestionsScreenState();
}

class _QuestionsScreenState extends State<QuestionsScreen> {
  String verdict = 'all';
  String qtype = 'all';
  String search = '';
  int renderCap = 200;
  QuestionDisplayRow? active;
  QuestionsView? data;
  bool loading = true;
  String? error;
  Timer? debounce;

  @override
  void initState() {
    super.initState();
    unawaited(fetch());
  }

  @override
  void didUpdateWidget(covariant QuestionsScreen oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.runId != widget.runId) {
      // Refresh the active drawer if it pointed at the previous run.
      active = null;
      unawaited(fetch());
    }
  }

  @override
  void dispose() {
    debounce?.cancel();
    super.dispose();
  }

  Future<void> fetch() async {
    setState(() {
      loading = true;
      error = null;
    });
    try {
      final next = await loadQuestions(
        id: widget.runId,
        verdict: verdict,
        qtype: qtype,
        search: search,
        renderCap: renderCap,
      );
      if (!mounted) return;
      setState(() => data = next);
    } catch (err) {
      if (mounted) setState(() => error = '$err');
    } finally {
      if (mounted) setState(() => loading = false);
    }
  }

  void debouncedSearch(String v) {
    search = v;
    debounce?.cancel();
    debounce = Timer(const Duration(milliseconds: 140), () {
      renderCap = 200;
      unawaited(fetch());
    });
  }

  @override
  Widget build(BuildContext context) {
    final d = data;
    return Column(
      children: [
        Container(
          padding: const EdgeInsets.all(8),
          decoration: const BoxDecoration(
            color: panel,
            border: Border(bottom: BorderSide(color: borderBright)),
          ),
          child: Row(
            children: [
              for (final item in const [
                ('all', 'ALL'),
                ('correct', 'CORRECT'),
                ('wrong', 'WRONG'),
                ('abstain', 'ABSTAIN'),
                ('error', 'ERROR'),
              ])
                _SegmentButton(
                  label: item.$2,
                  active: verdict == item.$1,
                  onTap: () {
                    verdict = item.$1;
                    renderCap = 200;
                    unawaited(fetch());
                  },
                ),
              const SizedBox(width: 10),
              DropdownButton<String>(
                value: qtype,
                dropdownColor: elev,
                style: body(text),
                items: [
                  for (final t in d?.types ?? const ['all'])
                    DropdownMenuItem(
                      value: t,
                      child: Text(t == 'all' ? 'ALL TYPES' : t),
                    ),
                ],
                onChanged: (v) {
                  if (v != null) {
                    qtype = v;
                    renderCap = 200;
                    unawaited(fetch());
                  }
                },
              ),
              const SizedBox(width: 10),
              SizedBox(
                width: 340,
                height: 30,
                child: TextField(
                  style: body(text),
                  decoration: fieldDecoration('search question / answer / id…'),
                  onChanged: debouncedSearch,
                ),
              ),
              const Spacer(),
              if (d != null)
                Text(
                  '${d.statsCorrect}   ${d.statsWrong}   ${d.matchCount}${d.hasMore ? ' · ${d.renderedCount}' : ''}',
                  style: body(dim),
                ),
            ],
          ),
        ),
        Expanded(
          child: error != null
              ? _ErrorState(error: error!, onRetry: fetch)
              : loading && d == null
              ? const _EmptyState('LOADING QUESTIONS…')
              : Stack(
                  children: [
                    Positioned.fill(
                      child: _QuestionTable(
                        rows: d?.rows ?? const [],
                        active: active,
                        onTap: (row) => setState(() => active = row),
                      ),
                    ),
                    if (d?.hasMore == true)
                      Positioned(
                        bottom: 12,
                        left: 0,
                        right: 0,
                        child: Center(
                          child: OutlinedButton(
                            onPressed: () {
                              renderCap += 200;
                              unawaited(fetch());
                            },
                            child: const Text('SHOW 200 MORE'),
                          ),
                        ),
                      ),
                    if (active != null)
                      QuestionDrawer(
                        runId: widget.runId,
                        row: active!,
                        onClose: () => setState(() => active = null),
                      ),
                  ],
                ),
        ),
      ],
    );
  }
}

class QuestionDrawer extends StatefulWidget {
  const QuestionDrawer({
    super.key,
    required this.runId,
    required this.row,
    required this.onClose,
  });
  final String runId;
  final QuestionDisplayRow row;
  final VoidCallback onClose;

  @override
  State<QuestionDrawer> createState() => _QuestionDrawerState();
}

class _QuestionDrawerState extends State<QuestionDrawer> {
  late Future<QuestionDebugView> future = loadQuestionDebug(
    id: widget.runId,
    debugArtifact: widget.row.debugArtifact,
  );
  final ScrollController _drawerScroll = ScrollController();

  @override
  void dispose() {
    _drawerScroll.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Positioned.fill(
      child: Container(
        margin: const EdgeInsets.fromLTRB(16, 10, 16, 12),
        decoration: BoxDecoration(
          color: panel,
          border: Border.all(color: borderBright),
          boxShadow: const [BoxShadow(color: Colors.black87, blurRadius: 40)],
        ),
        child: Column(
          children: [
            Container(
              height: 38,
              padding: const EdgeInsets.symmetric(horizontal: 10),
              decoration: const BoxDecoration(
                color: elev,
                border: Border(bottom: BorderSide(color: borderBright)),
              ),
              child: Row(
                children: [
                  KindChip(
                    label: widget.row.verdictKind.toUpperCase(),
                    color: verdictColor(widget.row.verdictKind),
                  ),
                  const SizedBox(width: 8),
                  Text(widget.row.questionId, style: mono(dim, 11)),
                  const Spacer(),
                  IconButton(
                    onPressed: widget.onClose,
                    icon: const Icon(Icons.close, size: 18, color: faint),
                  ),
                ],
              ),
            ),
            Expanded(
              child: FutureBuilder<QuestionDebugView>(
                future: future,
                builder: (context, snap) {
                  if (snap.connectionState != ConnectionState.done) {
                    return const _EmptyState('LOADING QUESTION DEBUG…');
                  }
                  if (snap.hasError) {
                    return _ErrorState(
                      error: '${snap.error}',
                      onRetry: () => setState(
                        () => future = loadQuestionDebug(
                          id: widget.runId,
                          debugArtifact: widget.row.debugArtifact,
                        ),
                      ),
                    );
                  }
                  final d = snap.requireData;
                  return Scrollbar(
                    controller: _drawerScroll,
                    notificationPredicate: (notification) =>
                        notification.depth == 0,
                    child: SingleChildScrollView(
                      controller: _drawerScroll,
                      primary: false,
                      padding: const EdgeInsets.all(14),
                      keyboardDismissBehavior:
                          ScrollViewKeyboardDismissBehavior.onDrag,
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          Panel(
                            title: 'Question',
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.stretch,
                              children: [
                                KvTable(
                                  rows: [
                                    KvRow(
                                      label: 'TYPE',
                                      value: widget.row.questionType,
                                    ),
                                    KvRow(
                                      label: 'ROUTE',
                                      value: widget.row.route,
                                    ),
                                    KvRow(
                                      label: 'DEBUG BUNDLE',
                                      value: widget.row.debugArtifact.isEmpty
                                          ? 'not recorded for this row'
                                          : widget.row.debugArtifact,
                                    ),
                                  ],
                                  monoValues: true,
                                ),
                                const SizedBox(height: 10),
                                _DebugBlock(
                                  label: 'QUESTION',
                                  body: widget.row.question,
                                ),
                                _DebugBlock(
                                  label: 'GOLD ANSWER',
                                  body: widget.row.goldAnswer,
                                ),
                                _DebugBlock(
                                  label: 'HYPOTHESIS',
                                  body: widget.row.hypothesis,
                                ),
                              ],
                            ),
                          ),
                          const SizedBox(height: 12),
                          for (final section in d.sections) ...[
                            Panel(
                              title: section.title,
                              tag: section.meta,
                              child: Column(
                                crossAxisAlignment: CrossAxisAlignment.stretch,
                                children: [
                                  for (final block in section.blocks)
                                    _DebugBlock(
                                      label: block.label,
                                      body: block.body,
                                    ),
                                ],
                              ),
                            ),
                            const SizedBox(height: 12),
                          ],
                        ],
                      ),
                    ),
                  );
                },
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class LiveScreen extends StatefulWidget {
  const LiveScreen({super.key, required this.runId, required this.active});
  final String runId;
  final bool active;

  @override
  State<LiveScreen> createState() => _LiveScreenState();
}

class _LiveScreenState extends State<LiveScreen> {
  LiveView? data;
  String? error;
  Timer? timer;

  @override
  void initState() {
    super.initState();
    if (widget.active) {
      unawaited(fetch());
      _startTimer();
    }
  }

  void _startTimer() {
    timer ??= Timer.periodic(
      const Duration(seconds: 2),
      (_) => fetch(silent: true),
    );
  }

  void _stopTimer() {
    timer?.cancel();
    timer = null;
  }

  @override
  void didUpdateWidget(covariant LiveScreen oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.active != widget.active) {
      if (widget.active) {
        unawaited(fetch());
        _startTimer();
      } else {
        _stopTimer();
      }
    }
    if (oldWidget.runId != widget.runId) {
      setState(() {
        data = null;
        error = null;
      });
      if (widget.active) unawaited(fetch());
    }
  }

  @override
  void dispose() {
    _stopTimer();
    super.dispose();
  }

  Future<void> fetch({bool silent = false}) async {
    // The bridge bootstrap blocks runApp() in main(), so __state is
    // already set by the time this runs. No per-call wait needed.
    try {
      final next = await loadLive(id: widget.runId);
      if (!mounted) return;
      setState(() {
        data = next;
        error = null;
      });
    } catch (err) {
      if (mounted) setState(() => error = '$err');
    }
  }

  @override
  Widget build(BuildContext context) {
    final d = data;
    if (d == null) {
      return _EmptyState(
        error == null ? 'READING LIVE STATE…' : 'ERROR: $error',
      );
    }
    return SingleChildScrollView(
      padding: const EdgeInsets.all(6),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 7),
            decoration: BoxDecoration(
              color: panel,
              border: Border.all(color: borderBright),
            ),
            child: Row(
              children: [
                KindChip(label: d.statusLabel, color: statusColor(d.status)),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        d.runName,
                        style: body(text).copyWith(fontWeight: FontWeight.w800),
                      ),
                      Text(d.meta, style: body(dim)),
                    ],
                  ),
                ),
                Text(
                  error == null ? d.age : '${d.age} · poll err',
                  style: body(faint),
                ),
              ],
            ),
          ),
          const SizedBox(height: 6),
          if (d.emptyMessage.isNotEmpty) ...[
            _Notice(d.emptyMessage),
            const SizedBox(height: 6),
          ],
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(
                child: Panel(
                  title: 'Progress',
                  tag: 'live',
                  child: _LiveProgress(data: d),
                ),
              ),
              const SizedBox(width: 6),
              Expanded(
                child: Panel(
                  title: 'Pipeline Drilldown',
                  tag: '${d.stageRows.length} stages',
                  child: _StageRows(rows: d.stageRows),
                ),
              ),
            ],
          ),
          const SizedBox(height: 6),
          if (d.errors.isNotEmpty) ...[
            Panel(
              title: 'Error Log',
              tag: '${d.errors.length} retained',
              flush: true,
              child: _TraceRows(rows: d.errors),
            ),
            const SizedBox(height: 6),
          ],
          Panel(
            title: 'Recent Activity',
            tag: '${d.activity.length} events',
            flush: true,
            child: d.activity.isEmpty
                ? const _EmptyState('✓ no activity in recent window')
                : _TraceRows(rows: d.activity),
          ),
          const SizedBox(height: 6),
          Text(
            '↻ auto-refreshing every 2s · live stats read from the run root (tailed)',
            style: mini(faint),
          ),
        ],
      ),
    );
  }
}

class _LiveProgress extends StatelessWidget {
  const _LiveProgress({required this.data});
  final LiveView data;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        for (final p in data.progress)
          Padding(
            padding: const EdgeInsets.only(bottom: 8),
            child: Row(
              children: [
                SizedBox(
                  width: 78,
                  child: Text(p.label, style: labelStyle(faint)),
                ),
                Expanded(
                  child: Bar(
                    value: p.ratio,
                    color: p.label == 'INGESTED' ? cyan : amber,
                  ),
                ),
                SizedBox(
                  width: 78,
                  child: Text(
                    p.value,
                    textAlign: TextAlign.right,
                    style: mono(text, 13),
                  ),
                ),
              ],
            ),
          ),
        _Tiles(rows: data.modelStats, columns: 5),
        const SizedBox(height: 8),
        _Tiles(rows: data.queueTiles, columns: 6),
        const SizedBox(height: 8),
        _SegmentBar(segments: data.queueSegments),
        const SizedBox(height: 6),
        for (final q in data.queueRows) _LiveQueue(row: q),
      ],
    );
  }
}

class _StageRows extends StatelessWidget {
  const _StageRows({required this.rows});
  final List<LiveStageRow> rows;

  @override
  Widget build(BuildContext context) {
    if (rows.isEmpty) return const _EmptyState('no memory operations yet');
    return Column(
      children: [
        for (final row in rows)
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 2),
            child: Row(
              children: [
                SizedBox(
                  width: 98,
                  child: Text(
                    row.name,
                    overflow: TextOverflow.ellipsis,
                    style: body(dim),
                  ),
                ),
                Expanded(
                  child: Tooltip(
                    message: row.title,
                    child: _SegmentBar(segments: row.segments, height: 8),
                  ),
                ),
                const SizedBox(width: 8),
                SizedBox(
                  width: 190,
                  child: Text(
                    row.count,
                    textAlign: TextAlign.right,
                    overflow: TextOverflow.ellipsis,
                    style: mono(row.hasFailed ? red : text, 9),
                  ),
                ),
              ],
            ),
          ),
        const SizedBox(height: 6),
        Row(
          children: [
            legend('done', green),
            legend('partial', cyan),
            legend('in-flight', amber),
            legend('failed', red),
          ],
        ),
      ],
    );
  }
}

class _LiveQueue extends StatelessWidget {
  const _LiveQueue({required this.row});
  final LiveQueueRow row;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(vertical: 4),
      decoration: const BoxDecoration(
        border: Border(top: BorderSide(color: border)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Expanded(
                child: Text(
                  row.name,
                  overflow: TextOverflow.ellipsis,
                  style: body(text),
                ),
              ),
              Text(row.operation.toUpperCase(), style: labelStyle(faint)),
            ],
          ),
          Wrap(
            spacing: 10,
            children: [
              for (final m in row.metrics)
                Text(
                  '${m.label} ${m.value}',
                  style: mono(
                    m.label == 'fail' && row.hasFailed ? red : dim,
                    9.5,
                  ),
                ),
            ],
          ),
        ],
      ),
    );
  }
}

class Panel extends StatelessWidget {
  const Panel({
    super.key,
    required this.title,
    required this.child,
    this.tag = '',
    this.flush = false,
    this.scroll = false,
    this.action,
  });
  final String title;
  final String tag;
  final Widget child;
  final bool flush;
  final bool scroll;
  final Widget? action;

  @override
  Widget build(BuildContext context) {
    final bodyChild = scroll ? SingleChildScrollView(child: child) : child;
    return Container(
      decoration: BoxDecoration(
        color: panel,
        border: Border.all(color: border),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Container(
            height: 32,
            padding: const EdgeInsets.symmetric(horizontal: 10),
            decoration: const BoxDecoration(
              color: elev,
              border: Border(bottom: BorderSide(color: border)),
            ),
            child: Row(
              children: [
                Text(title.toUpperCase(), style: labelStyle(text)),
                const Spacer(),
                if (action != null)
                  action!
                else
                  Text(
                    tag.toUpperCase(),
                    overflow: TextOverflow.ellipsis,
                    style: labelStyle(amber),
                  ),
              ],
            ),
          ),
          Flexible(
            fit: FlexFit.loose,
            child: Padding(
              padding: EdgeInsets.all(flush ? 0 : 12),
              child: bodyChild,
            ),
          ),
        ],
      ),
    );
  }
}

class RingGauge extends StatelessWidget {
  const RingGauge({
    super.key,
    required this.value,
    required this.label,
    this.size = 138,
  });
  final double value;
  final String label;
  final double size;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: size,
      height: size,
      child: Stack(
        alignment: Alignment.center,
        children: [
          CustomPaint(
            size: Size.square(size),
            painter: RingGaugePainter(value: value),
          ),
          Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(
                label,
                style: const TextStyle(
                  color: amber,
                  fontSize: 26,
                  fontWeight: FontWeight.w800,
                  fontFeatures: [FontFeature.tabularFigures()],
                ),
              ),
              const SizedBox(height: 3),
              Text('OVERALL', style: labelStyle(faint)),
            ],
          ),
        ],
      ),
    );
  }
}

class RingGaugePainter extends CustomPainter {
  RingGaugePainter({required this.value});
  final double value;
  @override
  void paint(Canvas canvas, Size size) {
    const stroke = 9.0;
    final rect =
        Offset(stroke / 2, stroke / 2) &
        Size(size.width - stroke, size.height - stroke);
    const start = 135 * math.pi / 180;
    const sweep = 270 * math.pi / 180;
    final bgPaint = Paint()
      ..color = elev
      ..style = PaintingStyle.stroke
      ..strokeWidth = stroke
      ..strokeCap = StrokeCap.round;
    final fgPaint = Paint()
      ..color = amber
      ..style = PaintingStyle.stroke
      ..strokeWidth = stroke
      ..strokeCap = StrokeCap.round;
    canvas.drawArc(rect, start, sweep, false, bgPaint);
    canvas.drawArc(rect, start, sweep * value.clamp(0.0, 1.0), false, fgPaint);
  }

  @override
  bool shouldRepaint(covariant RingGaugePainter oldDelegate) =>
      oldDelegate.value != value;
}

class Bar extends StatelessWidget {
  const Bar({
    super.key,
    required this.value,
    required this.color,
    this.height = 12,
  });
  final double value;
  final Color color;
  final double height;
  @override
  Widget build(BuildContext context) => Container(
    height: height,
    decoration: BoxDecoration(
      color: elev,
      border: Border.all(color: border),
    ),
    child: FractionallySizedBox(
      alignment: Alignment.centerLeft,
      widthFactor: value.clamp(0.0, 1.0),
      child: Container(color: color),
    ),
  );
}

class _SegmentBar extends StatelessWidget {
  const _SegmentBar({required this.segments, this.height = 10});
  final List<LiveSegment> segments;
  final double height;
  @override
  Widget build(BuildContext context) => Container(
    height: height,
    decoration: BoxDecoration(
      color: elev,
      border: Border.all(color: border),
    ),
    clipBehavior: Clip.hardEdge,
    child: Row(
      children: [
        for (final s in segments)
          if (s.pct > 0)
            Expanded(
              flex: s.pct.round().clamp(1, 1000),
              child: Tooltip(
                message: s.label,
                child: Container(color: segmentColor(s.kind)),
              ),
            ),
      ],
    ),
  );
}

class KvTable extends StatelessWidget {
  const KvTable({super.key, required this.rows, this.monoValues = false});
  final List<KvRow> rows;
  final bool monoValues;
  @override
  Widget build(BuildContext context) {
    if (rows.isEmpty) return const _EmptyState('—');
    return Table(
      columnWidths: const {0: IntrinsicColumnWidth(), 1: FlexColumnWidth()},
      defaultVerticalAlignment: TableCellVerticalAlignment.middle,
      children: [
        for (final row in rows)
          TableRow(
            children: [
              Padding(
                padding: const EdgeInsets.only(right: 12, bottom: 4),
                child: Text(row.label, style: labelStyle(faint)),
              ),
              Padding(
                padding: const EdgeInsets.only(bottom: 4),
                child: Text(
                  row.value,
                  overflow: TextOverflow.ellipsis,
                  style: monoValues ? mono(dim, 11) : body(text),
                ),
              ),
            ],
          ),
      ],
    );
  }
}

class _Tiles extends StatelessWidget {
  const _Tiles({required this.rows, required this.columns});
  final List<KvRow> rows;
  final int columns;
  @override
  Widget build(BuildContext context) => GridView.count(
    crossAxisCount: columns,
    childAspectRatio: 2.0,
    shrinkWrap: true,
    physics: const NeverScrollableScrollPhysics(),
    children: [for (final row in rows) _ScoreTile(row: row)],
  );
}

class _ScoreTile extends StatelessWidget {
  const _ScoreTile({required this.row});
  final KvRow row;
  @override
  Widget build(BuildContext context) => Container(
    width: 138,
    padding: const EdgeInsets.all(8),
    decoration: BoxDecoration(
      color: panel,
      border: Border.all(color: border),
    ),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(row.label, style: labelStyle(faint)),
        const Spacer(),
        Text(
          row.value,
          overflow: TextOverflow.ellipsis,
          style: mono(text, 17).copyWith(fontWeight: FontWeight.w700),
        ),
      ],
    ),
  );
}

class _Artifacts extends StatelessWidget {
  const _Artifacts({required this.rows});
  final List<ArtifactRow> rows;
  @override
  Widget build(BuildContext context) => Column(
    children: [
      for (final a in rows)
        Padding(
          padding: const EdgeInsets.symmetric(vertical: 2),
          child: Row(
            children: [
              Text(
                a.present ? '●' : '○',
                style: TextStyle(color: a.present ? green : faint),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Text(a.kind, style: body(a.present ? dim : faint)),
              ),
              Text(a.status, style: mini(faint)),
            ],
          ),
        ),
    ],
  );
}

class _ModelTable extends StatelessWidget {
  const _ModelTable({required this.rows});
  final List<ModelRow> rows;
  @override
  Widget build(BuildContext context) => _SimpleTable(
    headers: const ['Model', 'Calls', 'In', 'Out', 'Cost', 'p50'],
    rows: [
      for (final m in rows)
        [m.model, m.calls, m.inputTokens, m.outputTokens, m.cost, m.latency],
    ],
    flexes: const [3, 1, 1, 1, 1, 1],
  );
}

class _MemoryTable extends StatelessWidget {
  const _MemoryTable({required this.rows});
  final List<MemoryTimingRow> rows;
  @override
  Widget build(BuildContext context) => _StaticTable(
    maxHeight: 420,
    headers: const [
      'Operation',
      'Items',
      'Windows',
      'p50',
      'p80',
      'p95',
      'p98',
      'Subtimings',
      'Mid Err',
    ],
    rows: [
      for (final r in rows)
        [
          r.operation,
          r.items,
          r.windows,
          r.p50,
          r.p80,
          r.p95,
          r.p98,
          r.subtimings,
          r.midErr,
        ],
    ],
    flexes: const [2, 1, 1, 1, 1, 1, 1, 4, 1],
  );
}

class _QueueSummaryTable extends StatelessWidget {
  const _QueueSummaryTable({required this.rows});
  final List<QueueSummaryViewRow> rows;
  @override
  Widget build(BuildContext context) => rows.isEmpty
      ? const _EmptyState('No queues match the active filter.')
      : _SimpleTable(
          headers: const [
            'Queue',
            'Count',
            'Failed',
            'Wait p50/p80/p95/p98',
            'Run p50/p80/p95/p98',
            'Total p50/p80/p95/p98',
          ],
          rows: [
            for (final r in rows)
              [r.name, r.count, r.failed, r.wait, r.run, r.total],
          ],
          flexes: const [2, 1, 1, 2, 2, 2],
        );
}

class _QuestionTable extends StatefulWidget {
  const _QuestionTable({
    required this.rows,
    required this.active,
    required this.onTap,
  });

  final List<QuestionDisplayRow> rows;
  final QuestionDisplayRow? active;
  final ValueChanged<QuestionDisplayRow> onTap;

  @override
  State<_QuestionTable> createState() => _QuestionTableState();
}

class _QuestionTableState extends State<_QuestionTable> {
  final ScrollController _questionScroll = ScrollController();

  static const _headers = [
    'V',
    'ID',
    'Type',
    'Question',
    'Answer (gold)',
    'Hypothesis',
    'Route',
  ];
  static const _flexes = [1, 2, 2, 5, 4, 4, 2];

  @override
  void dispose() {
    _questionScroll.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    Widget cell(String value, TextStyle style) => Padding(
      padding: const EdgeInsets.symmetric(horizontal: 7),
      child: Text(value, overflow: TextOverflow.ellipsis, style: style),
    );

    Widget row(
      List<String> values, {
      bool header = false,
      bool active = false,
      int index = 0,
    }) {
      return InkWell(
        onTap: header ? null : () => widget.onTap(widget.rows[index]),
        child: Container(
          height: header ? 32 : 34,
          color: header
              ? panel
              : active
              ? selectedBg
              : (index.isEven ? rowBg : Colors.transparent),
          child: Row(
            children: [
              for (var i = 0; i < values.length; i++)
                Expanded(
                  flex: _flexes[i],
                  child: cell(
                    values[i],
                    header
                        ? labelStyle(faint)
                        : (i == 0 ? body(text) : body(dim)),
                  ),
                ),
            ],
          ),
        ),
      );
    }

    return Column(
      children: [
        row(_headers, header: true),
        Expanded(
          child: Scrollbar(
            controller: _questionScroll,
            notificationPredicate: (notification) => notification.depth == 0,
            child: ListView.builder(
              controller: _questionScroll,
              primary: false,
              itemExtent: 34,
              scrollCacheExtent: const ScrollCacheExtent.pixels(768),
              physics: const ClampingScrollPhysics(
                parent: BouncingScrollPhysics(),
              ),
              keyboardDismissBehavior: ScrollViewKeyboardDismissBehavior.onDrag,
              itemCount: widget.rows.length,
              itemBuilder: (context, index) {
                final r = widget.rows[index];
                return row(
                  [
                    r.verdict,
                    r.questionId,
                    r.questionType,
                    r.question,
                    r.goldAnswer,
                    r.hypothesis,
                    r.route,
                  ],
                  index: index,
                  active: widget.active?.questionId == r.questionId,
                );
              },
            ),
          ),
        ),
      ],
    );
  }
}

class _TraceRows extends StatelessWidget {
  const _TraceRows({required this.rows});
  final List<TraceEventRow> rows;
  @override
  Widget build(BuildContext context) => _StaticTable(
    maxHeight: 720,
    headers: const ['Time', 'Source', 'Operation', 'Status', 'Message'],
    rows: [
      for (final r in rows)
        [r.time, r.source, r.operation, r.status, r.message],
    ],
    flexes: const [1, 1, 2, 2, 6],
  );
}

/// Unbounded-height-safe table for scrollable / Column-embedded contexts.
/// Mirrors the fix from flutter-rinf-debugger: _SimpleTable uses
/// Expanded(child: ListView.builder) which collapses to zero height inside an
/// unbounded parent (SingleChildScrollView + Column). _StaticTable renders
/// rows as Column children — no Expanded, no ListView.
///
/// Scroll model (matches user expectation: "down first, then right"):
///   outer: vertical scroll over rows (trackpad/touch scrolls rows)
///   inner: horizontal scroll over columns only when content overflows the
///   panel width
/// Wrapped in a max-height so the table consumes only its panel — not the
/// whole page — letting the page-level scroll (TracesScreen etc.) reach
/// sibling panels like Bottleneck and Unified Trace Log without scrolling
/// through every trace row.
class _StaticTable extends StatelessWidget {
  const _StaticTable({
    required this.headers,
    required this.rows,
    required this.flexes,
    this.maxHeight = 420,
  });

  final List<String> headers;
  final List<List<String>> rows;
  final List<int> flexes;
  final double maxHeight;

  @override
  Widget build(BuildContext context) {
    Widget row(List<String> cells, {bool header = false, int index = 0}) {
      return Container(
        height: header ? 32 : 34,
        color: header ? panel : (index.isEven ? rowBg : Colors.transparent),
        child: Row(
          children: [
            for (var i = 0; i < cells.length; i++)
              Expanded(
                flex: flexes[i],
                child: Tooltip(
                  message: cells[i],
                  waitDuration: const Duration(milliseconds: 350),
                  child: Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 7),
                    child: Text(
                      cells[i],
                      overflow: TextOverflow.ellipsis,
                      style: header
                          ? labelStyle(faint)
                          : (i == 0 ? body(text) : body(dim)),
                    ),
                  ),
                ),
              ),
          ],
        ),
      );
    }

    // IMPORTANT: no horizontal scroll view here.
    // Trackpad/wheel vertical deltas must always scroll rows first. Previous
    // versions nested horizontal + vertical SingleChildScrollViews, and macOS
    // trackpad diagonal deltas made the table drift right/left before moving
    // down/up. The columns now flex to the available panel width and long text
    // ellipsizes with a Tooltip for the full value.
    return LayoutBuilder(
      builder: (context, constraints) {
        final tableWidth = constraints.hasBoundedWidth
            ? constraints.maxWidth
            : MediaQuery.of(context).size.width;
        return ConstrainedBox(
          constraints: BoxConstraints(maxHeight: maxHeight),
          child: Scrollbar(
            child: SingleChildScrollView(
              scrollDirection: Axis.vertical,
              child: SizedBox(
                width: tableWidth,
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    row(headers, header: true),
                    if (rows.isEmpty)
                      const Padding(
                        padding: EdgeInsets.all(16),
                        child: _EmptyState('—'),
                      )
                    else
                      for (var i = 0; i < rows.length; i++)
                        row(rows[i], index: i),
                  ],
                ),
              ),
            ),
          ),
        );
      },
    );
  }
}

class _SimpleTable extends StatelessWidget {
  const _SimpleTable({
    required this.headers,
    required this.rows,
    required this.flexes,
  });

  final List<String> headers;
  final List<List<String>> rows;
  final List<int> flexes;

  @override
  Widget build(BuildContext context) {
    Widget cell(String value, TextStyle style) {
      return Tooltip(
        message: value,
        waitDuration: const Duration(milliseconds: 350),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 7),
          child: Text(value, overflow: TextOverflow.ellipsis, style: style),
        ),
      );
    }

    Widget row(List<String> values, {bool header = false, int index = 0}) {
      return Container(
        height: header ? 32 : 34,
        color: header ? panel : (index.isEven ? rowBg : Colors.transparent),
        child: Row(
          children: [
            for (var i = 0; i < values.length; i++)
              Expanded(
                flex: flexes[i],
                child: cell(
                  values[i],
                  header
                      ? labelStyle(faint)
                      : (i == 0 ? body(text) : body(dim)),
                ),
              ),
          ],
        ),
      );
    }

    return LayoutBuilder(
      builder: (context, constraints) {
        final tableWidth = constraints.hasBoundedWidth
            ? constraints.maxWidth
            : MediaQuery.of(context).size.width;
        return SizedBox(
          width: tableWidth,
          child: Column(
            children: [
              row(headers, header: true),
              Expanded(
                child: Scrollbar(
                  child: ListView.builder(
                    itemExtent: 34,
                    scrollCacheExtent: const ScrollCacheExtent.pixels(512),
                    physics: const ClampingScrollPhysics(
                      parent: BouncingScrollPhysics(),
                    ),
                    keyboardDismissBehavior:
                        ScrollViewKeyboardDismissBehavior.onDrag,
                    itemCount: rows.length,
                    itemBuilder: (context, index) =>
                        row(rows[index], index: index),
                  ),
                ),
              ),
            ],
          ),
        );
      },
    );
  }
}

class _Bottlenecks extends StatelessWidget {
  const _Bottlenecks({required this.rows});
  final List<BottleneckRow> rows;
  @override
  Widget build(BuildContext context) => Column(
    children: [
      for (final r in rows)
        Padding(
          padding: const EdgeInsets.symmetric(vertical: 3),
          child: Row(
            children: [
              SizedBox(
                width: 260,
                child: Row(
                  children: [
                    KindChip(
                      label: r.kind,
                      color: r.kind == 'memory' ? cyan : amber,
                    ),
                    const SizedBox(width: 6),
                    Expanded(
                      child: Text(
                        r.name,
                        overflow: TextOverflow.ellipsis,
                        style: body(text),
                      ),
                    ),
                  ],
                ),
              ),
              Expanded(
                child: Container(
                  height: 11,
                  decoration: BoxDecoration(
                    color: elev,
                    border: Border.all(color: border),
                  ),
                  child: Row(
                    children: [
                      if (r.workPct > 0)
                        Expanded(
                          flex: r.workPct.round().clamp(1, 100),
                          child: Container(color: cyan),
                        ),
                      if (r.waitPct > 0)
                        Expanded(
                          flex: r.waitPct.round().clamp(1, 100),
                          child: Container(color: amber),
                        ),
                      if (r.runPct > 0)
                        Expanded(
                          flex: r.runPct.round().clamp(1, 100),
                          child: Container(color: green),
                        ),
                    ],
                  ),
                ),
              ),
              SizedBox(
                width: 120,
                child: Text(
                  r.label,
                  textAlign: TextAlign.right,
                  style: mono(text, 10),
                ),
              ),
              const SizedBox(width: 8),
              SizedBox(
                width: 260,
                child: Text(
                  r.meta,
                  overflow: TextOverflow.ellipsis,
                  style: mini(faint),
                ),
              ),
            ],
          ),
        ),
      Row(
        children: [
          legend('memory work', cyan),
          legend('provider wait', amber),
          legend('provider run', green),
        ],
      ),
    ],
  );
}

class _DebugBlock extends StatelessWidget {
  const _DebugBlock({required this.label, required this.body});
  final String label;
  final String body;
  @override
  Widget build(BuildContext context) => Container(
    margin: const EdgeInsets.only(bottom: 8),
    padding: const EdgeInsets.all(8),
    decoration: BoxDecoration(
      color: bg,
      border: Border.all(color: border),
    ),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(label, style: labelStyle(faint)),
        const SizedBox(height: 4),
        SelectableText(
          body.isEmpty ? '—' : body,
          style: mono(text, 11).copyWith(height: 1.4),
        ),
      ],
    ),
  );
}

class _TreeHeader extends StatelessWidget {
  const _TreeHeader({required this.label, this.trailing});
  final String label;
  final Widget? trailing;
  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 7),
    decoration: const BoxDecoration(
      border: Border(bottom: BorderSide(color: borderBright)),
    ),
    child: Row(
      children: [
        Text(label, style: labelStyle(faint)),
        const Spacer(),
        ?trailing,
      ],
    ),
  );
}

class _FilterBar extends StatelessWidget {
  const _FilterBar({required this.children});
  final List<Widget> children;
  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.fromLTRB(10, 5, 10, 0),
    child: Row(children: children),
  );
}

class _FilterChip extends StatelessWidget {
  const _FilterChip({
    required this.label,
    required this.active,
    required this.onTap,
  });
  final String label;
  final bool active;
  final VoidCallback onTap;
  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(right: 4),
    child: InkWell(
      onTap: onTap,
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
        decoration: BoxDecoration(
          color: active ? amber.withValues(alpha: .08) : rowBg,
          border: Border.all(color: active ? amberDim : border),
        ),
        child: Text(
          label.toUpperCase(),
          style: labelStyle(active ? amber : faint).copyWith(fontSize: 8.5),
        ),
      ),
    ),
  );
}

class _SegmentButton extends StatelessWidget {
  const _SegmentButton({
    required this.label,
    required this.active,
    required this.onTap,
  });
  final String label;
  final bool active;
  final VoidCallback onTap;
  @override
  Widget build(BuildContext context) => InkWell(
    onTap: onTap,
    child: Container(
      padding: const EdgeInsets.symmetric(horizontal: 9, vertical: 5),
      decoration: BoxDecoration(
        color: active ? amber.withValues(alpha: .10) : elev,
        border: Border.all(color: active ? amberDim : border),
      ),
      child: Text(label, style: labelStyle(active ? amber : dim)),
    ),
  );
}

class _StatusBar extends StatelessWidget {
  const _StatusBar({
    required this.status,
    required this.registry,
    required this.selectedId,
  });
  final String status;
  final RegistryView? registry;
  final String? selectedId;
  @override
  Widget build(BuildContext context) => Container(
    height: 28,
    padding: const EdgeInsets.symmetric(horizontal: 12),
    decoration: const BoxDecoration(
      color: panel,
      border: Border(top: BorderSide(color: border)),
    ),
    child: Row(
      children: [
        Text(
          apiReady ? 'API OK' : 'API WAIT',
          style: labelStyle(apiReady ? green : amber),
        ),
        const SizedBox(width: 14),
        Text(
          registry == null
              ? 'no registry'
              : '${registry!.runsTotal} runs · ${registry!.pendingTotal} pending',
          style: mini(dim),
        ),
        const SizedBox(width: 14),
        Expanded(
          child: Text(
            selectedId ?? 'no selection',
            overflow: TextOverflow.ellipsis,
            style: mini(dim),
          ),
        ),
        Text(status, overflow: TextOverflow.ellipsis, style: mini(faint)),
      ],
    ),
  );
}

class _Notice extends StatelessWidget {
  const _Notice(this.textValue);
  final String textValue;
  @override
  Widget build(BuildContext context) => Container(
    margin: const EdgeInsets.only(bottom: 10),
    padding: const EdgeInsets.all(10),
    decoration: BoxDecoration(
      color: panel,
      border: Border.all(color: borderBright),
    ),
    child: Text(textValue, style: body(dim)),
  );
}

class _EmptyState extends StatelessWidget {
  const _EmptyState(this.message);
  final String message;
  @override
  Widget build(BuildContext context) => Center(
    child: Padding(
      padding: const EdgeInsets.all(24),
      child: Text(
        message,
        textAlign: TextAlign.center,
        style: const TextStyle(color: faint, fontSize: 12, letterSpacing: 1.5),
      ),
    ),
  );
}

/// Crossfades only when the selected run changes. Tab clicks stay inside the
/// same `_TabView` instance so its lazy tab cache is preserved and heavy hidden
/// tabs are not rebuilt just to animate a click.
class _AnimatedRunBody extends StatelessWidget {
  const _AnimatedRunBody({
    required this.runId,
    required this.tab,
    required this.child,
  });
  final String runId;
  final String tab;
  final Widget child;
  @override
  Widget build(BuildContext context) {
    return AnimatedSwitcher(
      duration: const Duration(milliseconds: 180),
      switchInCurve: Curves.easeOutCubic,
      switchOutCurve: Curves.easeInCubic,
      layoutBuilder: (currentChild, previousChildren) => Stack(
        alignment: Alignment.topLeft,
        children: [...previousChildren, ?currentChild],
      ),
      transitionBuilder: (child, animation) =>
          FadeTransition(opacity: animation, child: child),
      child: KeyedSubtree(key: ValueKey(runId), child: child),
    );
  }
}

class _ErrorState extends StatelessWidget {
  const _ErrorState({required this.error, required this.onRetry});
  final String error;
  final VoidCallback onRetry;
  @override
  Widget build(BuildContext context) => Center(
    child: Container(
      width: 640,
      padding: const EdgeInsets.all(18),
      decoration: BoxDecoration(
        color: panel,
        border: Border.all(color: borderBright),
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          const Text(
            'LOAD FAILED',
            style: TextStyle(
              color: amber,
              fontWeight: FontWeight.w900,
              letterSpacing: 1.4,
            ),
          ),
          const SizedBox(height: 10),
          Text(error, style: body(dim), textAlign: TextAlign.center),
          const SizedBox(height: 14),
          OutlinedButton(onPressed: onRetry, child: const Text('RETRY')),
        ],
      ),
    ),
  );
}

class KindChip extends StatelessWidget {
  const KindChip({super.key, required this.label, required this.color});
  final String label;
  final Color color;
  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 3),
    decoration: BoxDecoration(
      color: color.withValues(alpha: .09),
      border: Border.all(color: color.withValues(alpha: .65)),
    ),
    child: Text(
      label,
      overflow: TextOverflow.ellipsis,
      style: labelStyle(color).copyWith(fontSize: 9),
    ),
  );
}

class _Dot extends StatelessWidget {
  const _Dot({required this.color});
  final Color color;
  @override
  Widget build(BuildContext context) => Container(
    width: 7,
    height: 7,
    decoration: BoxDecoration(
      color: color,
      shape: BoxShape.circle,
      boxShadow: [BoxShadow(color: color.withValues(alpha: .4), blurRadius: 5)],
    ),
  );
}

TextStyle labelStyle(Color color) => TextStyle(
  color: color,
  fontSize: 10,
  fontWeight: FontWeight.w900,
  letterSpacing: 1.0,
);
TextStyle mini(Color color) => TextStyle(color: color, fontSize: 10);
TextStyle body(Color color) => TextStyle(color: color, fontSize: 11.5);
TextStyle mono(Color color, double size) => TextStyle(
  color: color,
  fontSize: size,
  fontFeatures: const [FontFeature.tabularFigures()],
);

InputDecoration fieldDecoration(String hint) => InputDecoration(
  hintText: hint,
  hintStyle: mini(faint),
  contentPadding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
  filled: true,
  fillColor: bg,
  enabledBorder: const OutlineInputBorder(
    borderSide: BorderSide(color: border),
  ),
  focusedBorder: const OutlineInputBorder(borderSide: BorderSide(color: amber)),
);

Color statusColor(String status) => switch (status) {
  'running' => green,
  'warning' => amber,
  'complete' => cyan,
  _ => faint,
};
Color verdictColor(String kind) => switch (kind) {
  'correct' => green,
  'wrong' || 'error' => red,
  _ => faint,
};
Color dotColor(String kind) => switch (kind) {
  'native' => green,
  'trial' => amber,
  'tuning' => violet,
  _ => cyan,
};
Color segmentColor(String kind) => switch (kind) {
  'done' || 'succeeded' => green,
  'running' => amber,
  'queued' || 'partial' => cyan,
  'failed' || 'dead' => red,
  _ => elev,
};
Widget legend(String label, Color color) => Padding(
  padding: const EdgeInsets.only(right: 12),
  child: Row(
    mainAxisSize: MainAxisSize.min,
    children: [
      Container(width: 10, height: 8, color: color),
      const SizedBox(width: 4),
      Text(label, style: mini(faint)),
    ],
  ),
);
