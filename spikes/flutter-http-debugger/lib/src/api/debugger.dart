// Hand-written no-Rust data layer for the Flutter HTTP debugger spike.
// Keeps the generated FRB model/function surface so the mature UI can be reused,
// but talks directly to membench-server over HTTP.

import 'dart:convert';
import 'dart:math' as math;

import 'package:http/http.dart' as http;

import '../gen/membench/dashboard/v1/debugger.pb.dart' as pb;

const String _apiBase = String.fromEnvironment(
  'MEMBENCH_API',
  defaultValue: 'http://127.0.0.1:8787/api',
);

Uri _uri(String path, [Map<String, String?> query = const {}]) {
  final base = Uri.parse(_apiBase.endsWith('/') ? _apiBase : '$_apiBase/');
  return base
      .resolve(path)
      .replace(
        queryParameters: {
          for (final entry in query.entries)
            if (entry.value != null) entry.key: entry.value!,
        },
      );
}

Future<dynamic> _getJson(
  String path, [
  Map<String, String?> query = const {},
]) async {
  final uri = _uri(path, query);
  final res = await http.get(
    uri,
    headers: const {'Accept': 'application/json'},
  );
  if (res.statusCode < 200 || res.statusCode >= 300) {
    throw Exception('GET $uri failed ${res.statusCode}: ${res.body}');
  }
  return jsonDecode(res.body);
}

Future<T> _getPb<T>(
  String path,
  T Function(List<int>) decode, [
  Map<String, String?> query = const {},
]) async {
  final uri = _uri(path, query);
  final res = await http.get(
    uri,
    headers: const {'Accept': 'application/x-protobuf'},
  );
  if (res.statusCode < 200 || res.statusCode >= 300) {
    throw Exception('GET $uri failed ${res.statusCode}: ${res.body}');
  }
  return decode(res.bodyBytes);
}

String? _os(bool present, String value) => present ? value : null;
int? _oi(bool present, int value) => present ? value : null;
double? _od(bool present, double value) => present ? value : null;
bool? _ob(bool present, bool value) => present ? value : null;

Map<String, dynamic> _healthPbMap(pb.HealthResponse h) => {
  'ok': h.ok,
  'service': h.service,
  'version': h.version,
  'git_sha': h.gitSha,
  'binary_sha': h.binarySha,
};

Map<String, dynamic> _runSummaryPbMap(pb.RunSummary r) => {
  'run_id': r.runId,
  'origin': r.origin,
  'system': r.system,
  'benchmark': r.benchmark,
  'limit': _oi(r.hasLimit(), r.limit),
  'run_name': r.runName,
  'display_name': r.displayName,
  'run_kind': r.runKind,
  'registry_section': r.registrySection,
  'is_meta_record': r.isMetaRecord,
  'tuning_cohort': _os(r.hasTuningCohort(), r.tuningCohort),
  'tuning_shape': _os(r.hasTuningShape(), r.tuningShape),
  'config_label': r.configLabel,
  'settings_label': r.settingsLabel,
  'accuracy': _od(r.hasAccuracy(), r.accuracy),
  'accuracy_correct': _oi(r.hasAccuracyCorrect(), r.accuracyCorrect),
  'accuracy_total': _oi(r.hasAccuracyTotal(), r.accuracyTotal),
  'task_averaged_accuracy': _od(
    r.hasTaskAveragedAccuracy(),
    r.taskAveragedAccuracy,
  ),
  'abstention_accuracy': _od(r.hasAbstentionAccuracy(), r.abstentionAccuracy),
  'cost_micro_usd': r.hasCostMicroUsd() ? r.costMicroUsd.toInt() : null,
  'latency_ms_p50': _od(r.hasLatencyMsP50(), r.latencyMsP50),
  'latency_ms_p95': _od(r.hasLatencyMsP95(), r.latencyMsP95),
  'config_signature': _os(r.hasConfigSignature(), r.configSignature),
  'cohort_id': r.cohortId,
  'dataset_fingerprint': _os(r.hasDatasetFingerprint(), r.datasetFingerprint),
  'judge_model': _os(r.hasJudgeModel(), r.judgeModel),
  'judge_prompt_mode': _os(r.hasJudgePromptMode(), r.judgePromptMode),
  'oracle_gold': r.oracleGold,
  'created_at': _os(r.hasCreatedAt(), r.createdAt),
  'modified_ms': _od(r.hasModifiedMs(), r.modifiedMs),
  'per_question_type': r.perQuestionType.map(
    (key, score) => MapEntry(key, {
      'accuracy': score.accuracy,
      'n': score.n,
      'correct': score.correct,
      'total': score.total,
    }),
  ),
  'artifacts_available': r.artifactsAvailable,
  'artifacts_missing': r.artifactsMissing,
  'native_state_available': _ob(
    r.hasNativeStateAvailable(),
    r.nativeStateAvailable,
  ),
  'is_trial_run': r.isTrialRun,
};

Map<String, dynamic> _pendingRunPbMap(pb.PendingRun p) => {
  'age_secs': _od(p.hasAgeSecs(), p.ageSecs),
  'benchmark': p.benchmark,
  'config_label': p.configLabel,
  'hypotheses': p.hypotheses,
  'ingested': p.ingested,
  'limit': _oi(p.hasLimit(), p.limit),
  'oracle_gold': p.oracleGold,
  'origin': p.origin,
  'run_id': p.runId,
  'run_name': p.runName,
  'settings_label': p.settingsLabel,
  'started_ms': _od(p.hasStartedMs(), p.startedMs),
  'status': p.status,
  'system': p.system,
  'updated_ms': _od(p.hasUpdatedMs(), p.updatedMs),
};

Map<String, dynamic> _questionRowPbMap(pb.QuestionRow q) => {
  'question_id': q.questionId,
  'question_type': _os(q.hasQuestionType(), q.questionType),
  'question': _os(q.hasQuestion(), q.question),
  'gold_answer': _os(q.hasGoldAnswer(), q.goldAnswer),
  'hypothesis': _os(q.hasHypothesis(), q.hypothesis),
  'label': _ob(q.hasLabel(), q.label),
  'is_abstention': _ob(q.hasIsAbstention(), q.isAbstention),
  'judge_raw': _os(q.hasJudgeRaw(), q.judgeRaw),
  'judge_system_prompt': _os(q.hasJudgeSystemPrompt(), q.judgeSystemPrompt),
  'judge_user_prompt': _os(q.hasJudgeUserPrompt(), q.judgeUserPrompt),
  'judge_model': _os(q.hasJudgeModel(), q.judgeModel),
  'router_pick': _os(q.hasRouterPick(), q.routerPick),
  'initial_pick': _os(q.hasInitialPick(), q.initialPick),
  'final_pick': _os(q.hasFinalPick(), q.finalPick),
  'debug_artifact': _os(q.hasDebugArtifact(), q.debugArtifact),
  'error': _os(q.hasError(), q.error),
};

Future<Map<String, dynamic>> _healthMap() async {
  try {
    return _healthPbMap(
      await _getPb('health.pb', pb.HealthResponse.fromBuffer),
    );
  } catch (_) {
    return _map(await _getJson('health'));
  }
}

Future<List<Map<String, dynamic>>> _runsList() async {
  try {
    final response = await _getPb('runs.pb', pb.RunsResponse.fromBuffer);
    return response.runs.map(_runSummaryPbMap).toList();
  } catch (_) {
    final runsJson = _map(await _getJson('runs'));
    return _list(runsJson['runs']).map(_map).toList();
  }
}

Future<List<Map<String, dynamic>>> _pendingList() async {
  try {
    final response = await _getPb('pending.pb', pb.PendingResponse.fromBuffer);
    return response.pending.map(_pendingRunPbMap).toList();
  } catch (_) {
    final pendingJson = _map(await _getJson('pending'));
    return _list(pendingJson['pending']).map(_map).toList();
  }
}

Future<List<Map<String, dynamic>>> _questionRows(String id) async {
  try {
    final response = await _getPb(
      'run/questions.pb',
      pb.QuestionsResponse.fromBuffer,
      {'id': id},
    );
    return response.questions.map(_questionRowPbMap).toList();
  } catch (_) {
    final d = _map(await _getJson('run/questions', {'id': id}));
    return _list(d['questions']).map(_map).toList();
  }
}

Map<String, dynamic> _map(dynamic v) =>
    v is Map ? Map<String, dynamic>.from(v) : <String, dynamic>{};
List<dynamic> _list(dynamic v) => v is List ? v : const [];
String _s(dynamic v, [String fallback = '—']) {
  if (v == null) return fallback;
  final s = '$v';
  return s.isEmpty ? fallback : s;
}

String _empty(dynamic v) => v == null ? '' : '$v';
num? _n(dynamic v) => v is num ? v : (v is String ? num.tryParse(v) : null);
double _d(dynamic v, [double fallback = 0]) => (_n(v)?.toDouble()) ?? fallback;
int _i(dynamic v, [int fallback = 0]) => (_n(v)?.toInt()) ?? fallback;
bool _b(dynamic v, [bool fallback = false]) => v is bool ? v : fallback;

String pct(dynamic value) {
  final v = _n(value);
  if (v == null) return '—';
  final p = v.toDouble() * 100;
  return p >= 99.95 ? '100.0' : p.toStringAsFixed(1);
}

String money(dynamic microUsd) {
  final v = _n(microUsd);
  if (v == null) return '—';
  return '\$${(v / 1_000_000).toStringAsFixed(4)}';
}

String tokens(dynamic value) {
  final v = _n(value);
  if (v == null) return '—';
  final x = v.toDouble();
  if (x >= 1_000_000) return '${(x / 1_000_000).toStringAsFixed(1)}M';
  if (x >= 1_000) return '${(x / 1_000).toStringAsFixed(1)}K';
  return x.toStringAsFixed(0);
}

String ms(dynamic value) {
  final v = _n(value);
  if (v == null) return '—';
  final x = v.toDouble();
  if (x >= 1000) return '${(x / 1000).toStringAsFixed(1)}s';
  return '${x.toStringAsFixed(0)}ms';
}

String shortHash(dynamic v) {
  final s = _empty(v);
  if (s.length <= 8) return s.isEmpty ? '—' : s;
  return s.substring(0, 8);
}

String shortQueue(dynamic v) {
  final s = _empty(v);
  final parts = s.split(':');
  if (parts.length >= 2) return '${parts.first}:${parts.last}';
  return s.isEmpty ? '—' : s;
}

String _ago(dynamic msEpoch) {
  final v = _n(msEpoch)?.toDouble();
  if (v == null || v == 0) return '—';
  final now = DateTime.now().millisecondsSinceEpoch.toDouble();
  final sec = ((now - v) / 1000).clamp(0, double.infinity);
  if (sec < 60) return '${sec.toStringAsFixed(0)}s';
  if (sec < 3600) return '${(sec / 60).toStringAsFixed(0)}m';
  if (sec < 86400) return '${(sec / 3600).toStringAsFixed(1)}h';
  return '${(sec / 86400).toStringAsFixed(1)}d';
}

String _timeOnly(dynamic timestamp) {
  final s = _empty(timestamp);
  if (s.length >= 19 && s.contains('T')) return s.substring(11, 19);
  return s.isEmpty ? '—' : s;
}

String _pretty(dynamic v) => const JsonEncoder.withIndent('  ').convert(v);
String _inline(dynamic v) {
  if (v == null) return '—';
  if (v is String) return v.isEmpty ? '—' : v;
  if (v is num || v is bool) return '$v';
  return jsonEncode(v);
}

Future<BridgeHealth> bridgeHealth() async {
  final health = await _healthMap();
  return BridgeHealth(
    version: _s(health['version'], '0.1.0'),
    apiBaseUrl: _apiBase,
    ok: _b(health['ok'], true),
  );
}

Future<RegistryView> loadRegistry({
  required String sort,
  required bool showBenchmarks,
  required bool showTuning,
  required bool showTrials,
  required bool showRuns,
  required bool showRecords,
  required bool showStale,
}) async {
  final runs = await _runsList();
  final pending = await _pendingList();
  final pendingNodes = pending
      .where((p) => showStale || _s(p['status'], '') != 'stalled')
      .map(
        (p) => PendingNode(
          runId: _s(p['run_id'], ''),
          runName: _s(p['run_name'], ''),
          status: _s(p['status'], 'unknown'),
          progress: '${_i(p['ingested'])}/${p['limit'] ?? '?'}',
        ),
      )
      .toList();

  bool sectionAllowed(Map<String, dynamic> r) {
    final section = _s(r['registry_section'], 'benchmarks');
    if (section == 'tuning') return showTuning;
    if (section == 'trials') return showTrials;
    return showBenchmarks;
  }

  bool originAllowed(Map<String, dynamic> r) {
    final origin = _s(r['origin'], 'runs');
    return origin == 'records' ? showRecords : showRuns;
  }

  final filtered = runs
      .where((r) => sectionAllowed(r) && originAllowed(r))
      .toList();
  int cmp(Map<String, dynamic> a, Map<String, dynamic> b) {
    final at = _d(a['modified_ms']);
    final bt = _d(b['modified_ms']);
    if (sort == 'newest') return bt.compareTo(at);
    if (sort == 'oldest') return at.compareTo(bt);
    final aa = _d(a['accuracy'], -1);
    final ba = _d(b['accuracy'], -1);
    final c = ba.compareTo(aa);
    return c != 0 ? c : bt.compareTo(at);
  }

  filtered.sort(cmp);

  final grouped =
      <
        String,
        ({String label, String sublabel, List<Map<String, dynamic>> runs})
      >{};
  for (final r in filtered) {
    final source = _s(r['origin'], 'runs');
    final limit = _s(r['limit'], '?');
    final section = _s(r['registry_section'], 'benchmarks');
    late String key;
    late String label;
    late String sublabel;
    if (section == 'tuning') {
      final cohort = _s(r['tuning_cohort'], 'embedding transport');
      key = '1:tuning:$source:$cohort:$limit';
      label = 'tuning · $source';
      sublabel = '$cohort / ${limit}Q';
    } else if (section == 'trials') {
      key = '2:trials:$source:${_s(r['system'])}:${_s(r['benchmark'])}:$limit';
      label = 'trials · $source';
      sublabel = '${_s(r['system'])} / ${_s(r['benchmark'])} / ${limit}Q';
    } else {
      key = '3:bench:$source:${_s(r['system'])}:${_s(r['benchmark'])}:$limit';
      label = 'benchmark · $source';
      sublabel = '${_s(r['system'])} / ${_s(r['benchmark'])} / ${limit}Q';
    }
    grouped
        .putIfAbsent(
          key,
          () => (
            label: label,
            sublabel: sublabel,
            runs: <Map<String, dynamic>>[],
          ),
        )
        .runs
        .add(r);
  }

  final groups = grouped.entries.map((e) {
    final rows = e.value.runs..sort(cmp);
    return RegistryGroup(
      key: e.key,
      label: e.value.label,
      sublabel: e.value.sublabel,
      runs: rows.map((r) {
        final section = _s(r['registry_section'], 'benchmarks');
        return RegistryRunNode(
          runId: _s(r['run_id'], ''),
          name: _s(r['display_name'], _s(r['run_name'], '—')),
          meta: sort == 'score' ? pct(r['accuracy']) : _ago(r['modified_ms']),
          sourceBadge: _b(r['is_meta_record'])
              ? 'META'
              : (_s(r['origin']) == 'records' ? 'REC' : 'RUN'),
          runKind: _s(r['run_kind'], 'native').toUpperCase(),
          dotKind: section == 'tuning'
              ? 'tuning'
              : (_b(r['is_trial_run'])
                    ? 'trial'
                    : (_s(r['run_kind']) == 'native' ? 'native' : 'default')),
          nativeStateAvailable: _b(r['native_state_available']),
        );
      }).toList(),
    );
  }).toList();
  groups.sort((a, b) => a.key.compareTo(b.key));

  return RegistryView(
    runsTotal: runs.length,
    hiddenRegistryCount: runs.length - filtered.length,
    pendingTotal: pending.length,
    runningCount: pending.where((p) => _s(p['status'], '') == 'running').length,
    warningCount: pending.where((p) => _s(p['status'], '') == 'warning').length,
    staleCount: pending.where((p) => _s(p['status'], '') == 'stalled').length,
    pending: pendingNodes,
    groups: groups,
    defaultRunId: groups.expand((g) => g.runs).isEmpty
        ? ''
        : groups.expand((g) => g.runs).first.runId,
  );
}

Future<OverviewView> loadOverview({required String id}) async {
  final d = _map(await _getJson('run', {'id': id}));
  final s = _map(d['summary']);
  final cohort = _map(d['cohort']);
  final params = _map(d['params']);
  final cost = _map(d['cost']);
  final report = _map(d['report']);
  final manifest = _map(_map(report['artifact_manifest']));
  final available = _list(s['artifacts_available']).map((e) => '$e').toSet();
  final missing = _list(s['artifacts_missing']).map((e) => '$e').toSet();
  final artifacts = [...available, ...missing]
      .map(
        (kind) => ArtifactRow(
          kind: kind,
          status: available.contains(kind) ? 'present' : 'missing',
          present: available.contains(kind),
        ),
      )
      .toList();
  if (artifacts.isEmpty && manifest.isNotEmpty) {
    artifacts.addAll(
      _list(
        manifest['available'],
      ).map((k) => ArtifactRow(kind: '$k', status: 'present', present: true)),
    );
  }
  final models = _list(cost['models'])
      .map(_map)
      .map(
        (m) => ModelRow(
          model: _s(m['model'], _s(m['name'], '—')),
          sub: _s(m['role'], _s(m['provider'], '—')),
          calls: tokens(m['calls']),
          inputTokens: tokens(m['input_tokens']),
          outputTokens: tokens(m['output_tokens']),
          cost: money(m['cost_micro_usd']),
          latency: ms(m['latency_ms_p50']),
        ),
      )
      .toList();
  final modelMap = _map(cohort['models']);
  return OverviewView(
    runId: id,
    runName: _s(s['display_name'], _s(s['run_name'], id)),
    runKind: _s(s['run_kind'], 'native'),
    accuracyValue: _d(s['accuracy']),
    accuracyLabel: pct(s['accuracy']),
    tiles: [
      KvRow(
        label: 'Correct',
        value: '${_i(s['accuracy_correct'])}/${_i(s['accuracy_total'])}',
      ),
      KvRow(label: 'Task Avg', value: pct(s['task_averaged_accuracy'])),
      KvRow(label: 'Abstention', value: pct(s['abstention_accuracy'])),
      KvRow(label: 'Cost', value: money(s['cost_micro_usd'])),
      KvRow(label: 'Limit', value: _s(s['limit'])),
      KvRow(label: 'Modified', value: _ago(s['modified_ms'])),
    ],
    cohort: [
      KvRow(label: 'System', value: _s(s['system'])),
      KvRow(label: 'Benchmark', value: _s(s['benchmark'])),
      KvRow(label: 'Dataset', value: shortHash(s['dataset_fingerprint'])),
      KvRow(label: 'Judge', value: _s(s['judge_model'])),
      KvRow(label: 'Prompt', value: _s(s['judge_prompt_mode'])),
      for (final entry in modelMap.entries)
        KvRow(label: entry.key.toUpperCase(), value: _s(entry.value)),
    ],
    params: params.entries
        .take(120)
        .map((e) => KvRow(label: e.key, value: _inline(e.value)))
        .toList(),
    artifacts: artifacts,
    modelRows: models,
  );
}

Future<QuestionsView> loadQuestions({
  required String id,
  required String verdict,
  required String qtype,
  required String search,
  required int renderCap,
}) async {
  final rows = await _questionRows(id);
  final correct = rows.where((r) => r['label'] == true).length;
  final wrong = rows.where((r) => r['label'] == false).length;
  final types = <String>{
    'all',
    ...rows.map((r) => _empty(r['question_type'])).where((s) => s.isNotEmpty),
  }.toList();
  final needle = search.trim().toLowerCase();
  bool match(Map<String, dynamic> r) {
    if (verdict == 'correct' && r['label'] != true) return false;
    if (verdict == 'wrong' && r['label'] != false) return false;
    if (verdict == 'abstain' && r['is_abstention'] != true) return false;
    if (verdict == 'error' && _empty(r['error']).isEmpty) return false;
    if (qtype.trim().isNotEmpty &&
        qtype != 'all' &&
        _empty(r['question_type']) != qtype) {
      return false;
    }
    if (needle.isNotEmpty) {
      final hay =
          '${r['question']} ${r['gold_answer']} ${r['hypothesis']} ${r['question_id']}'
              .toLowerCase();
      if (!hay.contains(needle)) return false;
    }
    return true;
  }

  final filtered = rows.where(match).toList();
  final cap = math.max(1, renderCap);
  return QuestionsView(
    rows: filtered.take(cap).map((r) => _questionRow(r)).toList(),
    types: types,
    statsCorrect: '$correct✓',
    statsWrong: '$wrong✗',
    matchCount: '${filtered.length}/${rows.length} matches',
    renderedCount: '${math.min(cap, filtered.length)} shown',
    hasMore: filtered.length > cap,
  );
}

QuestionDisplayRow _questionRow(Map<String, dynamic> r) {
  final label = r['label'];
  final kind = label == true
      ? 'correct'
      : (label == false
            ? 'wrong'
            : (_b(r['is_abstention'])
                  ? 'abstain'
                  : (_empty(r['error']).isNotEmpty ? 'error' : 'unknown')));
  return QuestionDisplayRow(
    questionId: _s(r['question_id'], ''),
    verdict: kind == 'correct'
        ? '✓'
        : (kind == 'wrong'
              ? '✗'
              : kind == 'abstain'
              ? '∅'
              : '!'),
    verdictKind: kind,
    questionType: _s(r['question_type'], '—'),
    question: _s(r['question'], ''),
    goldAnswer: _s(r['gold_answer'], ''),
    hypothesis: _s(r['hypothesis'], ''),
    route: _s(
      r['final_pick'],
      _s(r['router_pick'], _s(r['initial_pick'], '—')),
    ),
    debugArtifact: _s(r['debug_artifact'], ''),
  );
}

Future<QuestionDebugView> loadQuestionDebug({
  required String id,
  required String debugArtifact,
}) async {
  if (debugArtifact.trim().isEmpty) {
    return const QuestionDebugView(
      title: 'No question selected',
      headerKind: 'unknown',
      rows: [],
      sections: [],
    );
  }
  final d = _map(
    await _getJson('run/question-debug', {'id': id, 'path': debugArtifact}),
  );
  final json = d['json'];
  final root = _map(json);
  final sections = <DebugSection>[];
  for (final entry in root.entries) {
    final value = entry.value;
    if (value is Map) {
      sections.add(
        DebugSection(
          title: entry.key,
          meta: '${value.length} fields',
          blocks: value.entries
              .map((e) => DebugBlock(label: '${e.key}', body: _pretty(e.value)))
              .toList(),
        ),
      );
    } else if (value is List) {
      sections.add(
        DebugSection(
          title: entry.key,
          meta: '${value.length} items',
          blocks: [DebugBlock(label: entry.key, body: _pretty(value))],
        ),
      );
    } else {
      sections.add(
        DebugSection(
          title: entry.key,
          meta: '',
          blocks: [DebugBlock(label: entry.key, body: _inline(value))],
        ),
      );
    }
  }
  return QuestionDebugView(
    title: 'Question Debug',
    headerKind: 'debug',
    rows: [KvRow(label: 'PATH', value: _s(d['path'], debugArtifact))],
    sections: sections,
  );
}

Future<TracesView> loadTraces({
  required String id,
  required String queueFilter,
  required bool showQueueSummary,
}) async {
  final d = _map(await _getJson('run/traces', {'id': id}));
  final memory = _list(d['memory_stage_timing'])
      .map(_map)
      .map(
        (r) => MemoryTimingRow(
          operation: _s(r['operation'], '—'),
          items: '${_i(r['item_count'])} ${_s(r['item_unit'], 'items')}',
          windows: '${_i(r['events'])} ev · ${_i(r['batch_events'])} batches',
          p50: ms(r['work_ms_p50']),
          p80: ms(r['work_ms_p80']),
          p95: ms(r['work_ms_p95']),
          p98: ms(r['work_ms_p98']),
          subtimings: _subtimings(_map(r['numeric_metrics'])),
          midErr: '${_i(r['intermediate_failed'])}/${_i(r['failed'])}',
          hasError: _i(r['failed']) > 0 || _i(r['intermediate_failed']) > 0,
        ),
      )
      .toList();
  final queueRows = showQueueSummary
      ? _queueSummary(_list(d['queue_timing']).map(_map).toList(), queueFilter)
      : <QueueSummaryViewRow>[];
  final traceRows = _list(
    _map(d['trace_events'])['rows'],
  ).map(_map).take(500).map(_traceRow).toList();
  final workflow = _workflowRows(_map(d['workflow_queue']));
  final bottlenecks = [...memory]
    ..sort((a, b) => _parseMs(b.p98).compareTo(_parseMs(a.p98)));
  return TracesView(
    noneMessage: memory.isEmpty && queueRows.isEmpty && traceRows.isEmpty
        ? 'No trace artifacts for this run.'
        : '',
    memoryRows: memory,
    queueRows: queueRows,
    bottlenecks: bottlenecks
        .take(8)
        .map(
          (r) => BottleneckRow(
            kind: 'memory',
            name: r.operation,
            label: r.p98,
            meta: r.items,
            workPct: _parseMs(r.p98).clamp(0, 10000) / 100,
            waitPct: 0,
            runPct: 0,
          ),
        )
        .toList(),
    workflowRows: workflow,
    traceRows: traceRows,
  );
}

double _parseMs(String s) {
  if (s.endsWith('ms')) {
    return double.tryParse(s.substring(0, s.length - 2)) ?? 0;
  }
  if (s.endsWith('s')) {
    return (double.tryParse(s.substring(0, s.length - 1)) ?? 0) * 1000;
  }
  return 0;
}

String _subtimings(Map<String, dynamic> metrics) {
  if (metrics.isEmpty) return '—';
  final entries = metrics.entries.toList()
    ..sort(
      (a, b) => _d(_map(b.value)['p98']).compareTo(_d(_map(a.value)['p98'])),
    );
  return entries
      .take(4)
      .map((e) => '${e.key}:${ms(_map(e.value)['p98'])}')
      .join(' · ');
}

List<QueueSummaryViewRow> _queueSummary(
  List<Map<String, dynamic>> rows,
  String filter,
) {
  final groups = <String, List<Map<String, dynamic>>>{};
  for (final r in rows) {
    final name = '${_s(r['operation'], '')}:${shortQueue(r['queue_id'])}';
    if (filter.trim().isNotEmpty &&
        !name.toLowerCase().contains(filter.toLowerCase())) {
      continue;
    }
    groups.putIfAbsent(name, () => []).add(r);
  }
  double? percentile(List<double> vals, double p) {
    if (vals.isEmpty) return null;
    vals.sort();
    final idx = ((p / 100) * (vals.length - 1)).round().clamp(
      0,
      vals.length - 1,
    );
    return vals[idx];
  }

  final out = <QueueSummaryViewRow>[];
  for (final entry in groups.entries) {
    final vals = entry.value;
    List<double> col(String k) =>
        vals.map((r) => _d(r[k], double.nan)).where((x) => x.isFinite).toList();
    final failed = vals
        .where((r) => {'failed', 'dead'}.contains(_s(r['final_status'], '')))
        .length;
    out.add(
      QueueSummaryViewRow(
        name: entry.key,
        count: '${vals.length}',
        failed: '$failed',
        wait:
            'p50 ${ms(percentile(col('wait_ms'), 50))} · p95 ${ms(percentile(col('wait_ms'), 95))}',
        run:
            'p50 ${ms(percentile(col('run_ms'), 50))} · p95 ${ms(percentile(col('run_ms'), 95))}',
        total:
            'p50 ${ms(percentile(col('total_ms'), 50))} · p95 ${ms(percentile(col('total_ms'), 95))}',
        hasFailed: failed > 0,
      ),
    );
  }
  return out..sort((a, b) => b.name.compareTo(a.name));
}

List<KvRow> _workflowRows(Map<String, dynamic> w) {
  final dbs = _list(w['databases']).map(_map).toList();
  final rows = <KvRow>[];
  for (final db in dbs.take(4)) {
    rows.add(
      KvRow(
        label: _s(db['path'], 'workflow'),
        value:
            'queues ${_inline(db['queues'])} · status ${_inline(db['items_by_status'])}',
      ),
    );
  }
  return rows;
}

TraceEventRow _traceRow(Map<String, dynamic> r) => TraceEventRow(
  time: _timeOnly(r['timestamp']),
  source: _s(r['source'], _s(r['kind'], '—')),
  operation: _s(r['operation'], _s(r['lane'], '—')),
  status: _s(r['status'], _s(r['event'], '—')),
  message: _s(r['message'], _s(r['error'], _s(r['event'], ''))),
  severity: _s(r['severity'], _s(r['status'], 'info')),
);

Future<LiveView> loadLive({required String id}) async {
  final d = _map(await _getJson('run/live', {'id': id}));
  final p = _map(d['pending']);
  final detail = _map(d['detail']);
  final q = _map(detail['queue']);
  final model = _map(detail['model']);
  final limit = _i(p['limit']);
  final ingested = _i(p['ingested']);
  final hypotheses = _i(p['hypotheses']);
  final progress = limit == 0
      ? 0.0
      : (math.max(ingested, hypotheses) / limit).clamp(0.0, 1.0);
  final queues = _list(detail['queues'])
      .map(_map)
      .map(
        (row) => LiveQueueRow(
          name: shortQueue(row['queue_id']),
          operation: _s(row['operation'], '—'),
          metrics: [
            KvRow(label: 'queued', value: _s(row['queued'], '0')),
            KvRow(label: 'running', value: _s(row['running'], '0')),
            KvRow(label: 'done', value: _s(row['succeeded'], '0')),
            KvRow(label: 'failed', value: _s(row['failed'], '0')),
          ],
          hasFailed: _i(row['failed']) + _i(row['dead']) > 0,
        ),
      )
      .toList();
  final stages = _list(detail['memory_stages'])
      .map(_map)
      .map(
        (s) => LiveStageRow(
          name: _s(s['name'], _s(s['operation'], 'stage')),
          title: _s(s['title'], _s(s['operation'], 'stage')),
          count: '${_i(s['succeeded'])}/${_i(s['total'], _i(s['count']))}',
          segments: [
            LiveSegment(label: 'done', pct: _d(s['succeeded']), kind: 'ok'),
            LiveSegment(label: 'failed', pct: _d(s['failed']), kind: 'bad'),
          ],
          hasFailed: _i(s['failed']) > 0,
        ),
      )
      .toList();
  return LiveView(
    status: _s(p['status'], 'completed'),
    statusLabel: _s(p['status'], 'completed').toUpperCase(),
    runName: _s(p['run_name'], id),
    meta: '${_s(p['system'])} / ${_s(p['benchmark'])} / ${_s(p['limit'])}Q',
    age: _s(p['age_secs'], '—'),
    emptyMessage: detail.isEmpty ? 'No live detail for this run.' : '',
    progress: [
      ProgressRow(
        label: 'ingested',
        value: '$ingested/$limit',
        ratio: limit == 0 ? 0 : ingested / limit,
      ),
      ProgressRow(
        label: 'hypotheses',
        value: '$hypotheses/$limit',
        ratio: progress,
      ),
    ],
    modelStats: [
      KvRow(label: 'input', value: tokens(model['input_tokens'])),
      KvRow(label: 'output', value: tokens(model['output_tokens'])),
      KvRow(label: 'calls', value: tokens(model['window_calls'])),
      KvRow(label: 'failed', value: tokens(model['window_failed'])),
    ],
    queueTiles: [
      for (final e in q.entries) KvRow(label: e.key, value: _s(e.value)),
    ],
    queueSegments: [
      LiveSegment(label: 'queued', pct: _d(q['queued']), kind: 'queued'),
      LiveSegment(label: 'running', pct: _d(q['running']), kind: 'run'),
      LiveSegment(label: 'done', pct: _d(q['succeeded']), kind: 'ok'),
      LiveSegment(
        label: 'failed',
        pct: _d(q['failed']) + _d(q['dead']),
        kind: 'bad',
      ),
    ],
    queueRows: queues,
    stageRows: stages,
    errors: _list(detail['errors']).map(_map).take(80).map(_traceRow).toList(),
    activity: _list(
      detail['activity'],
    ).map(_map).take(120).map(_traceRow).toList(),
  );
}

class ArtifactRow {
  final String kind;
  final String status;
  final bool present;

  const ArtifactRow({
    required this.kind,
    required this.status,
    required this.present,
  });

  @override
  int get hashCode => kind.hashCode ^ status.hashCode ^ present.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is ArtifactRow &&
          runtimeType == other.runtimeType &&
          kind == other.kind &&
          status == other.status &&
          present == other.present;
}

class BottleneckRow {
  final String kind;
  final String name;
  final String label;
  final String meta;
  final double workPct;
  final double waitPct;
  final double runPct;

  const BottleneckRow({
    required this.kind,
    required this.name,
    required this.label,
    required this.meta,
    required this.workPct,
    required this.waitPct,
    required this.runPct,
  });

  @override
  int get hashCode =>
      kind.hashCode ^
      name.hashCode ^
      label.hashCode ^
      meta.hashCode ^
      workPct.hashCode ^
      waitPct.hashCode ^
      runPct.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is BottleneckRow &&
          runtimeType == other.runtimeType &&
          kind == other.kind &&
          name == other.name &&
          label == other.label &&
          meta == other.meta &&
          workPct == other.workPct &&
          waitPct == other.waitPct &&
          runPct == other.runPct;
}

class BridgeHealth {
  final String version;
  final String apiBaseUrl;
  final bool ok;

  const BridgeHealth({
    required this.version,
    required this.apiBaseUrl,
    required this.ok,
  });

  @override
  int get hashCode => version.hashCode ^ apiBaseUrl.hashCode ^ ok.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is BridgeHealth &&
          runtimeType == other.runtimeType &&
          version == other.version &&
          apiBaseUrl == other.apiBaseUrl &&
          ok == other.ok;
}

class DebugBlock {
  final String label;
  final String body;

  const DebugBlock({required this.label, required this.body});

  @override
  int get hashCode => label.hashCode ^ body.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is DebugBlock &&
          runtimeType == other.runtimeType &&
          label == other.label &&
          body == other.body;
}

class DebugSection {
  final String title;
  final String meta;
  final List<DebugBlock> blocks;

  const DebugSection({
    required this.title,
    required this.meta,
    required this.blocks,
  });

  @override
  int get hashCode => title.hashCode ^ meta.hashCode ^ blocks.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is DebugSection &&
          runtimeType == other.runtimeType &&
          title == other.title &&
          meta == other.meta &&
          blocks == other.blocks;
}

class KvRow {
  final String label;
  final String value;

  const KvRow({required this.label, required this.value});

  @override
  int get hashCode => label.hashCode ^ value.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is KvRow &&
          runtimeType == other.runtimeType &&
          label == other.label &&
          value == other.value;
}

class LiveQueueRow {
  final String name;
  final String operation;
  final List<KvRow> metrics;
  final bool hasFailed;

  const LiveQueueRow({
    required this.name,
    required this.operation,
    required this.metrics,
    required this.hasFailed,
  });

  @override
  int get hashCode =>
      name.hashCode ^
      operation.hashCode ^
      metrics.hashCode ^
      hasFailed.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is LiveQueueRow &&
          runtimeType == other.runtimeType &&
          name == other.name &&
          operation == other.operation &&
          metrics == other.metrics &&
          hasFailed == other.hasFailed;
}

class LiveSegment {
  final String label;
  final double pct;
  final String kind;

  const LiveSegment({
    required this.label,
    required this.pct,
    required this.kind,
  });

  @override
  int get hashCode => label.hashCode ^ pct.hashCode ^ kind.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is LiveSegment &&
          runtimeType == other.runtimeType &&
          label == other.label &&
          pct == other.pct &&
          kind == other.kind;
}

class LiveStageRow {
  final String name;
  final String title;
  final String count;
  final List<LiveSegment> segments;
  final bool hasFailed;

  const LiveStageRow({
    required this.name,
    required this.title,
    required this.count,
    required this.segments,
    required this.hasFailed,
  });

  @override
  int get hashCode =>
      name.hashCode ^
      title.hashCode ^
      count.hashCode ^
      segments.hashCode ^
      hasFailed.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is LiveStageRow &&
          runtimeType == other.runtimeType &&
          name == other.name &&
          title == other.title &&
          count == other.count &&
          segments == other.segments &&
          hasFailed == other.hasFailed;
}

class LiveView {
  final String status;
  final String statusLabel;
  final String runName;
  final String meta;
  final String age;
  final String emptyMessage;
  final List<ProgressRow> progress;
  final List<KvRow> modelStats;
  final List<KvRow> queueTiles;
  final List<LiveSegment> queueSegments;
  final List<LiveQueueRow> queueRows;
  final List<LiveStageRow> stageRows;
  final List<TraceEventRow> errors;
  final List<TraceEventRow> activity;

  const LiveView({
    required this.status,
    required this.statusLabel,
    required this.runName,
    required this.meta,
    required this.age,
    required this.emptyMessage,
    required this.progress,
    required this.modelStats,
    required this.queueTiles,
    required this.queueSegments,
    required this.queueRows,
    required this.stageRows,
    required this.errors,
    required this.activity,
  });

  @override
  int get hashCode =>
      status.hashCode ^
      statusLabel.hashCode ^
      runName.hashCode ^
      meta.hashCode ^
      age.hashCode ^
      emptyMessage.hashCode ^
      progress.hashCode ^
      modelStats.hashCode ^
      queueTiles.hashCode ^
      queueSegments.hashCode ^
      queueRows.hashCode ^
      stageRows.hashCode ^
      errors.hashCode ^
      activity.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is LiveView &&
          runtimeType == other.runtimeType &&
          status == other.status &&
          statusLabel == other.statusLabel &&
          runName == other.runName &&
          meta == other.meta &&
          age == other.age &&
          emptyMessage == other.emptyMessage &&
          progress == other.progress &&
          modelStats == other.modelStats &&
          queueTiles == other.queueTiles &&
          queueSegments == other.queueSegments &&
          queueRows == other.queueRows &&
          stageRows == other.stageRows &&
          errors == other.errors &&
          activity == other.activity;
}

class MemoryTimingRow {
  final String operation;
  final String items;
  final String windows;
  final String p50;
  final String p80;
  final String p95;
  final String p98;
  final String subtimings;
  final String midErr;
  final bool hasError;

  const MemoryTimingRow({
    required this.operation,
    required this.items,
    required this.windows,
    required this.p50,
    required this.p80,
    required this.p95,
    required this.p98,
    required this.subtimings,
    required this.midErr,
    required this.hasError,
  });

  @override
  int get hashCode =>
      operation.hashCode ^
      items.hashCode ^
      windows.hashCode ^
      p50.hashCode ^
      p80.hashCode ^
      p95.hashCode ^
      p98.hashCode ^
      subtimings.hashCode ^
      midErr.hashCode ^
      hasError.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is MemoryTimingRow &&
          runtimeType == other.runtimeType &&
          operation == other.operation &&
          items == other.items &&
          windows == other.windows &&
          p50 == other.p50 &&
          p80 == other.p80 &&
          p95 == other.p95 &&
          p98 == other.p98 &&
          subtimings == other.subtimings &&
          midErr == other.midErr &&
          hasError == other.hasError;
}

class ModelRow {
  final String model;
  final String sub;
  final String calls;
  final String inputTokens;
  final String outputTokens;
  final String cost;
  final String latency;

  const ModelRow({
    required this.model,
    required this.sub,
    required this.calls,
    required this.inputTokens,
    required this.outputTokens,
    required this.cost,
    required this.latency,
  });

  @override
  int get hashCode =>
      model.hashCode ^
      sub.hashCode ^
      calls.hashCode ^
      inputTokens.hashCode ^
      outputTokens.hashCode ^
      cost.hashCode ^
      latency.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is ModelRow &&
          runtimeType == other.runtimeType &&
          model == other.model &&
          sub == other.sub &&
          calls == other.calls &&
          inputTokens == other.inputTokens &&
          outputTokens == other.outputTokens &&
          cost == other.cost &&
          latency == other.latency;
}

class OverviewView {
  final String runId;
  final String runName;
  final String runKind;
  final double accuracyValue;
  final String accuracyLabel;
  final List<KvRow> tiles;
  final List<KvRow> cohort;
  final List<KvRow> params;
  final List<ArtifactRow> artifacts;
  final List<ModelRow> modelRows;

  const OverviewView({
    required this.runId,
    required this.runName,
    required this.runKind,
    required this.accuracyValue,
    required this.accuracyLabel,
    required this.tiles,
    required this.cohort,
    required this.params,
    required this.artifacts,
    required this.modelRows,
  });

  @override
  int get hashCode =>
      runId.hashCode ^
      runName.hashCode ^
      runKind.hashCode ^
      accuracyValue.hashCode ^
      accuracyLabel.hashCode ^
      tiles.hashCode ^
      cohort.hashCode ^
      params.hashCode ^
      artifacts.hashCode ^
      modelRows.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is OverviewView &&
          runtimeType == other.runtimeType &&
          runId == other.runId &&
          runName == other.runName &&
          runKind == other.runKind &&
          accuracyValue == other.accuracyValue &&
          accuracyLabel == other.accuracyLabel &&
          tiles == other.tiles &&
          cohort == other.cohort &&
          params == other.params &&
          artifacts == other.artifacts &&
          modelRows == other.modelRows;
}

class PendingNode {
  final String runId;
  final String runName;
  final String status;
  final String progress;

  const PendingNode({
    required this.runId,
    required this.runName,
    required this.status,
    required this.progress,
  });

  @override
  int get hashCode =>
      runId.hashCode ^ runName.hashCode ^ status.hashCode ^ progress.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is PendingNode &&
          runtimeType == other.runtimeType &&
          runId == other.runId &&
          runName == other.runName &&
          status == other.status &&
          progress == other.progress;
}

class ProgressRow {
  final String label;
  final String value;
  final double ratio;

  const ProgressRow({
    required this.label,
    required this.value,
    required this.ratio,
  });

  @override
  int get hashCode => label.hashCode ^ value.hashCode ^ ratio.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is ProgressRow &&
          runtimeType == other.runtimeType &&
          label == other.label &&
          value == other.value &&
          ratio == other.ratio;
}

class QuestionDebugView {
  final String title;
  final String headerKind;
  final List<KvRow> rows;
  final List<DebugSection> sections;

  const QuestionDebugView({
    required this.title,
    required this.headerKind,
    required this.rows,
    required this.sections,
  });

  @override
  int get hashCode =>
      title.hashCode ^ headerKind.hashCode ^ rows.hashCode ^ sections.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is QuestionDebugView &&
          runtimeType == other.runtimeType &&
          title == other.title &&
          headerKind == other.headerKind &&
          rows == other.rows &&
          sections == other.sections;
}

class QuestionDisplayRow {
  final String questionId;
  final String verdict;
  final String verdictKind;
  final String questionType;
  final String question;
  final String goldAnswer;
  final String hypothesis;
  final String route;
  final String debugArtifact;

  const QuestionDisplayRow({
    required this.questionId,
    required this.verdict,
    required this.verdictKind,
    required this.questionType,
    required this.question,
    required this.goldAnswer,
    required this.hypothesis,
    required this.route,
    required this.debugArtifact,
  });

  @override
  int get hashCode =>
      questionId.hashCode ^
      verdict.hashCode ^
      verdictKind.hashCode ^
      questionType.hashCode ^
      question.hashCode ^
      goldAnswer.hashCode ^
      hypothesis.hashCode ^
      route.hashCode ^
      debugArtifact.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is QuestionDisplayRow &&
          runtimeType == other.runtimeType &&
          questionId == other.questionId &&
          verdict == other.verdict &&
          verdictKind == other.verdictKind &&
          questionType == other.questionType &&
          question == other.question &&
          goldAnswer == other.goldAnswer &&
          hypothesis == other.hypothesis &&
          route == other.route &&
          debugArtifact == other.debugArtifact;
}

class QuestionsView {
  final List<QuestionDisplayRow> rows;
  final List<String> types;
  final String statsCorrect;
  final String statsWrong;
  final String matchCount;
  final String renderedCount;
  final bool hasMore;

  const QuestionsView({
    required this.rows,
    required this.types,
    required this.statsCorrect,
    required this.statsWrong,
    required this.matchCount,
    required this.renderedCount,
    required this.hasMore,
  });

  @override
  int get hashCode =>
      rows.hashCode ^
      types.hashCode ^
      statsCorrect.hashCode ^
      statsWrong.hashCode ^
      matchCount.hashCode ^
      renderedCount.hashCode ^
      hasMore.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is QuestionsView &&
          runtimeType == other.runtimeType &&
          rows == other.rows &&
          types == other.types &&
          statsCorrect == other.statsCorrect &&
          statsWrong == other.statsWrong &&
          matchCount == other.matchCount &&
          renderedCount == other.renderedCount &&
          hasMore == other.hasMore;
}

class QueueSummaryViewRow {
  final String name;
  final String count;
  final String failed;
  final String wait;
  final String run;
  final String total;
  final bool hasFailed;

  const QueueSummaryViewRow({
    required this.name,
    required this.count,
    required this.failed,
    required this.wait,
    required this.run,
    required this.total,
    required this.hasFailed,
  });

  @override
  int get hashCode =>
      name.hashCode ^
      count.hashCode ^
      failed.hashCode ^
      wait.hashCode ^
      run.hashCode ^
      total.hashCode ^
      hasFailed.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is QueueSummaryViewRow &&
          runtimeType == other.runtimeType &&
          name == other.name &&
          count == other.count &&
          failed == other.failed &&
          wait == other.wait &&
          run == other.run &&
          total == other.total &&
          hasFailed == other.hasFailed;
}

class RegistryGroup {
  final String key;
  final String label;
  final String sublabel;
  final List<RegistryRunNode> runs;

  const RegistryGroup({
    required this.key,
    required this.label,
    required this.sublabel,
    required this.runs,
  });

  @override
  int get hashCode =>
      key.hashCode ^ label.hashCode ^ sublabel.hashCode ^ runs.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is RegistryGroup &&
          runtimeType == other.runtimeType &&
          key == other.key &&
          label == other.label &&
          sublabel == other.sublabel &&
          runs == other.runs;
}

class RegistryRunNode {
  final String runId;
  final String name;
  final String meta;
  final String sourceBadge;
  final String runKind;
  final String dotKind;
  final bool nativeStateAvailable;

  const RegistryRunNode({
    required this.runId,
    required this.name,
    required this.meta,
    required this.sourceBadge,
    required this.runKind,
    required this.dotKind,
    required this.nativeStateAvailable,
  });

  @override
  int get hashCode =>
      runId.hashCode ^
      name.hashCode ^
      meta.hashCode ^
      sourceBadge.hashCode ^
      runKind.hashCode ^
      dotKind.hashCode ^
      nativeStateAvailable.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is RegistryRunNode &&
          runtimeType == other.runtimeType &&
          runId == other.runId &&
          name == other.name &&
          meta == other.meta &&
          sourceBadge == other.sourceBadge &&
          runKind == other.runKind &&
          dotKind == other.dotKind &&
          nativeStateAvailable == other.nativeStateAvailable;
}

class RegistryView {
  final int runsTotal;
  final int hiddenRegistryCount;
  final int pendingTotal;
  final int runningCount;
  final int warningCount;
  final int staleCount;
  final List<PendingNode> pending;
  final List<RegistryGroup> groups;
  final String defaultRunId;

  const RegistryView({
    required this.runsTotal,
    required this.hiddenRegistryCount,
    required this.pendingTotal,
    required this.runningCount,
    required this.warningCount,
    required this.staleCount,
    required this.pending,
    required this.groups,
    required this.defaultRunId,
  });

  @override
  int get hashCode =>
      runsTotal.hashCode ^
      hiddenRegistryCount.hashCode ^
      pendingTotal.hashCode ^
      runningCount.hashCode ^
      warningCount.hashCode ^
      staleCount.hashCode ^
      pending.hashCode ^
      groups.hashCode ^
      defaultRunId.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is RegistryView &&
          runtimeType == other.runtimeType &&
          runsTotal == other.runsTotal &&
          hiddenRegistryCount == other.hiddenRegistryCount &&
          pendingTotal == other.pendingTotal &&
          runningCount == other.runningCount &&
          warningCount == other.warningCount &&
          staleCount == other.staleCount &&
          pending == other.pending &&
          groups == other.groups &&
          defaultRunId == other.defaultRunId;
}

class TraceEventRow {
  final String time;
  final String source;
  final String operation;
  final String status;
  final String message;
  final String severity;

  const TraceEventRow({
    required this.time,
    required this.source,
    required this.operation,
    required this.status,
    required this.message,
    required this.severity,
  });

  @override
  int get hashCode =>
      time.hashCode ^
      source.hashCode ^
      operation.hashCode ^
      status.hashCode ^
      message.hashCode ^
      severity.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is TraceEventRow &&
          runtimeType == other.runtimeType &&
          time == other.time &&
          source == other.source &&
          operation == other.operation &&
          status == other.status &&
          message == other.message &&
          severity == other.severity;
}

class TracesView {
  final String noneMessage;
  final List<MemoryTimingRow> memoryRows;
  final List<QueueSummaryViewRow> queueRows;
  final List<BottleneckRow> bottlenecks;
  final List<KvRow> workflowRows;
  final List<TraceEventRow> traceRows;

  const TracesView({
    required this.noneMessage,
    required this.memoryRows,
    required this.queueRows,
    required this.bottlenecks,
    required this.workflowRows,
    required this.traceRows,
  });

  @override
  int get hashCode =>
      noneMessage.hashCode ^
      memoryRows.hashCode ^
      queueRows.hashCode ^
      bottlenecks.hashCode ^
      workflowRows.hashCode ^
      traceRows.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is TracesView &&
          runtimeType == other.runtimeType &&
          noneMessage == other.noneMessage &&
          memoryRows == other.memoryRows &&
          queueRows == other.queueRows &&
          bottlenecks == other.bottlenecks &&
          workflowRows == other.workflowRows &&
          traceRows == other.traceRows;
}
