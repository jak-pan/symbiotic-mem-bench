// This is a generated file - do not edit.
//
// Generated from membench/dashboard/v1/debugger.proto.

// @dart = 3.3

// ignore_for_file: annotate_overrides, camel_case_types, comment_references
// ignore_for_file: constant_identifier_names
// ignore_for_file: curly_braces_in_flow_control_structures
// ignore_for_file: deprecated_member_use_from_same_package, library_prefixes
// ignore_for_file: non_constant_identifier_names, prefer_relative_imports

import 'dart:core' as $core;

import 'package:fixnum/fixnum.dart' as $fixnum;
import 'package:protobuf/protobuf.dart' as $pb;

export 'package:protobuf/protobuf.dart' show GeneratedMessageGenericExtensions;

class HealthResponse extends $pb.GeneratedMessage {
  factory HealthResponse({
    $core.bool? ok,
    $core.String? service,
    $core.String? version,
    $core.String? gitSha,
    $core.String? binarySha,
  }) {
    final result = create();
    if (ok != null) result.ok = ok;
    if (service != null) result.service = service;
    if (version != null) result.version = version;
    if (gitSha != null) result.gitSha = gitSha;
    if (binarySha != null) result.binarySha = binarySha;
    return result;
  }

  HealthResponse._();

  factory HealthResponse.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory HealthResponse.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'HealthResponse',
      package: const $pb.PackageName(
          _omitMessageNames ? '' : 'membench.dashboard.v1'),
      createEmptyInstance: create)
    ..aOB(1, _omitFieldNames ? '' : 'ok')
    ..aOS(2, _omitFieldNames ? '' : 'service')
    ..aOS(3, _omitFieldNames ? '' : 'version')
    ..aOS(4, _omitFieldNames ? '' : 'gitSha')
    ..aOS(5, _omitFieldNames ? '' : 'binarySha')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  HealthResponse clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  HealthResponse copyWith(void Function(HealthResponse) updates) =>
      super.copyWith((message) => updates(message as HealthResponse))
          as HealthResponse;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static HealthResponse create() => HealthResponse._();
  @$core.override
  HealthResponse createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static HealthResponse getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<HealthResponse>(create);
  static HealthResponse? _defaultInstance;

  @$pb.TagNumber(1)
  $core.bool get ok => $_getBF(0);
  @$pb.TagNumber(1)
  set ok($core.bool value) => $_setBool(0, value);
  @$pb.TagNumber(1)
  $core.bool hasOk() => $_has(0);
  @$pb.TagNumber(1)
  void clearOk() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get service => $_getSZ(1);
  @$pb.TagNumber(2)
  set service($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasService() => $_has(1);
  @$pb.TagNumber(2)
  void clearService() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.String get version => $_getSZ(2);
  @$pb.TagNumber(3)
  set version($core.String value) => $_setString(2, value);
  @$pb.TagNumber(3)
  $core.bool hasVersion() => $_has(2);
  @$pb.TagNumber(3)
  void clearVersion() => $_clearField(3);

  @$pb.TagNumber(4)
  $core.String get gitSha => $_getSZ(3);
  @$pb.TagNumber(4)
  set gitSha($core.String value) => $_setString(3, value);
  @$pb.TagNumber(4)
  $core.bool hasGitSha() => $_has(3);
  @$pb.TagNumber(4)
  void clearGitSha() => $_clearField(4);

  @$pb.TagNumber(5)
  $core.String get binarySha => $_getSZ(4);
  @$pb.TagNumber(5)
  set binarySha($core.String value) => $_setString(4, value);
  @$pb.TagNumber(5)
  $core.bool hasBinarySha() => $_has(4);
  @$pb.TagNumber(5)
  void clearBinarySha() => $_clearField(5);
}

class QTypeScore extends $pb.GeneratedMessage {
  factory QTypeScore({
    $core.double? accuracy,
    $core.int? n,
    $core.int? correct,
    $core.int? total,
  }) {
    final result = create();
    if (accuracy != null) result.accuracy = accuracy;
    if (n != null) result.n = n;
    if (correct != null) result.correct = correct;
    if (total != null) result.total = total;
    return result;
  }

  QTypeScore._();

  factory QTypeScore.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory QTypeScore.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'QTypeScore',
      package: const $pb.PackageName(
          _omitMessageNames ? '' : 'membench.dashboard.v1'),
      createEmptyInstance: create)
    ..aD(1, _omitFieldNames ? '' : 'accuracy')
    ..aI(2, _omitFieldNames ? '' : 'n', fieldType: $pb.PbFieldType.OU3)
    ..aI(3, _omitFieldNames ? '' : 'correct', fieldType: $pb.PbFieldType.OU3)
    ..aI(4, _omitFieldNames ? '' : 'total', fieldType: $pb.PbFieldType.OU3)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  QTypeScore clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  QTypeScore copyWith(void Function(QTypeScore) updates) =>
      super.copyWith((message) => updates(message as QTypeScore)) as QTypeScore;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static QTypeScore create() => QTypeScore._();
  @$core.override
  QTypeScore createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static QTypeScore getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<QTypeScore>(create);
  static QTypeScore? _defaultInstance;

  @$pb.TagNumber(1)
  $core.double get accuracy => $_getN(0);
  @$pb.TagNumber(1)
  set accuracy($core.double value) => $_setDouble(0, value);
  @$pb.TagNumber(1)
  $core.bool hasAccuracy() => $_has(0);
  @$pb.TagNumber(1)
  void clearAccuracy() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.int get n => $_getIZ(1);
  @$pb.TagNumber(2)
  set n($core.int value) => $_setUnsignedInt32(1, value);
  @$pb.TagNumber(2)
  $core.bool hasN() => $_has(1);
  @$pb.TagNumber(2)
  void clearN() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.int get correct => $_getIZ(2);
  @$pb.TagNumber(3)
  set correct($core.int value) => $_setUnsignedInt32(2, value);
  @$pb.TagNumber(3)
  $core.bool hasCorrect() => $_has(2);
  @$pb.TagNumber(3)
  void clearCorrect() => $_clearField(3);

  @$pb.TagNumber(4)
  $core.int get total => $_getIZ(3);
  @$pb.TagNumber(4)
  set total($core.int value) => $_setUnsignedInt32(3, value);
  @$pb.TagNumber(4)
  $core.bool hasTotal() => $_has(3);
  @$pb.TagNumber(4)
  void clearTotal() => $_clearField(4);
}

class TrialMarker extends $pb.GeneratedMessage {
  factory TrialMarker({
    $core.String? stackId,
    $core.String? changeId,
    $core.String? changeTitle,
    $core.String? decision,
    $core.String? analysisPath,
    $core.String? comparedToRunId,
    $core.String? originalBaselineRunId,
    $core.int? improvements,
    $core.int? regressions,
    $core.int? unchangedWrong,
    $core.int? unchangedCorrect,
    $core.int? questionCount,
    $core.String? sampleClassification,
    $core.bool? focused,
    $core.double? aggregateAccuracy,
    $core.int? aggregateCorrect,
    $core.int? aggregateTotal,
  }) {
    final result = create();
    if (stackId != null) result.stackId = stackId;
    if (changeId != null) result.changeId = changeId;
    if (changeTitle != null) result.changeTitle = changeTitle;
    if (decision != null) result.decision = decision;
    if (analysisPath != null) result.analysisPath = analysisPath;
    if (comparedToRunId != null) result.comparedToRunId = comparedToRunId;
    if (originalBaselineRunId != null)
      result.originalBaselineRunId = originalBaselineRunId;
    if (improvements != null) result.improvements = improvements;
    if (regressions != null) result.regressions = regressions;
    if (unchangedWrong != null) result.unchangedWrong = unchangedWrong;
    if (unchangedCorrect != null) result.unchangedCorrect = unchangedCorrect;
    if (questionCount != null) result.questionCount = questionCount;
    if (sampleClassification != null)
      result.sampleClassification = sampleClassification;
    if (focused != null) result.focused = focused;
    if (aggregateAccuracy != null) result.aggregateAccuracy = aggregateAccuracy;
    if (aggregateCorrect != null) result.aggregateCorrect = aggregateCorrect;
    if (aggregateTotal != null) result.aggregateTotal = aggregateTotal;
    return result;
  }

  TrialMarker._();

  factory TrialMarker.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory TrialMarker.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'TrialMarker',
      package: const $pb.PackageName(
          _omitMessageNames ? '' : 'membench.dashboard.v1'),
      createEmptyInstance: create)
    ..aOS(1, _omitFieldNames ? '' : 'stackId')
    ..aOS(2, _omitFieldNames ? '' : 'changeId')
    ..aOS(3, _omitFieldNames ? '' : 'changeTitle')
    ..aOS(4, _omitFieldNames ? '' : 'decision')
    ..aOS(5, _omitFieldNames ? '' : 'analysisPath')
    ..aOS(6, _omitFieldNames ? '' : 'comparedToRunId')
    ..aOS(7, _omitFieldNames ? '' : 'originalBaselineRunId')
    ..aI(8, _omitFieldNames ? '' : 'improvements',
        fieldType: $pb.PbFieldType.OU3)
    ..aI(9, _omitFieldNames ? '' : 'regressions',
        fieldType: $pb.PbFieldType.OU3)
    ..aI(10, _omitFieldNames ? '' : 'unchangedWrong',
        fieldType: $pb.PbFieldType.OU3)
    ..aI(11, _omitFieldNames ? '' : 'unchangedCorrect',
        fieldType: $pb.PbFieldType.OU3)
    ..aI(12, _omitFieldNames ? '' : 'questionCount',
        fieldType: $pb.PbFieldType.OU3)
    ..aOS(13, _omitFieldNames ? '' : 'sampleClassification')
    ..aOB(14, _omitFieldNames ? '' : 'focused')
    ..aD(15, _omitFieldNames ? '' : 'aggregateAccuracy')
    ..aI(16, _omitFieldNames ? '' : 'aggregateCorrect',
        fieldType: $pb.PbFieldType.OU3)
    ..aI(17, _omitFieldNames ? '' : 'aggregateTotal',
        fieldType: $pb.PbFieldType.OU3)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  TrialMarker clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  TrialMarker copyWith(void Function(TrialMarker) updates) =>
      super.copyWith((message) => updates(message as TrialMarker))
          as TrialMarker;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static TrialMarker create() => TrialMarker._();
  @$core.override
  TrialMarker createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static TrialMarker getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<TrialMarker>(create);
  static TrialMarker? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get stackId => $_getSZ(0);
  @$pb.TagNumber(1)
  set stackId($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasStackId() => $_has(0);
  @$pb.TagNumber(1)
  void clearStackId() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get changeId => $_getSZ(1);
  @$pb.TagNumber(2)
  set changeId($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasChangeId() => $_has(1);
  @$pb.TagNumber(2)
  void clearChangeId() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.String get changeTitle => $_getSZ(2);
  @$pb.TagNumber(3)
  set changeTitle($core.String value) => $_setString(2, value);
  @$pb.TagNumber(3)
  $core.bool hasChangeTitle() => $_has(2);
  @$pb.TagNumber(3)
  void clearChangeTitle() => $_clearField(3);

  @$pb.TagNumber(4)
  $core.String get decision => $_getSZ(3);
  @$pb.TagNumber(4)
  set decision($core.String value) => $_setString(3, value);
  @$pb.TagNumber(4)
  $core.bool hasDecision() => $_has(3);
  @$pb.TagNumber(4)
  void clearDecision() => $_clearField(4);

  @$pb.TagNumber(5)
  $core.String get analysisPath => $_getSZ(4);
  @$pb.TagNumber(5)
  set analysisPath($core.String value) => $_setString(4, value);
  @$pb.TagNumber(5)
  $core.bool hasAnalysisPath() => $_has(4);
  @$pb.TagNumber(5)
  void clearAnalysisPath() => $_clearField(5);

  @$pb.TagNumber(6)
  $core.String get comparedToRunId => $_getSZ(5);
  @$pb.TagNumber(6)
  set comparedToRunId($core.String value) => $_setString(5, value);
  @$pb.TagNumber(6)
  $core.bool hasComparedToRunId() => $_has(5);
  @$pb.TagNumber(6)
  void clearComparedToRunId() => $_clearField(6);

  @$pb.TagNumber(7)
  $core.String get originalBaselineRunId => $_getSZ(6);
  @$pb.TagNumber(7)
  set originalBaselineRunId($core.String value) => $_setString(6, value);
  @$pb.TagNumber(7)
  $core.bool hasOriginalBaselineRunId() => $_has(6);
  @$pb.TagNumber(7)
  void clearOriginalBaselineRunId() => $_clearField(7);

  @$pb.TagNumber(8)
  $core.int get improvements => $_getIZ(7);
  @$pb.TagNumber(8)
  set improvements($core.int value) => $_setUnsignedInt32(7, value);
  @$pb.TagNumber(8)
  $core.bool hasImprovements() => $_has(7);
  @$pb.TagNumber(8)
  void clearImprovements() => $_clearField(8);

  @$pb.TagNumber(9)
  $core.int get regressions => $_getIZ(8);
  @$pb.TagNumber(9)
  set regressions($core.int value) => $_setUnsignedInt32(8, value);
  @$pb.TagNumber(9)
  $core.bool hasRegressions() => $_has(8);
  @$pb.TagNumber(9)
  void clearRegressions() => $_clearField(9);

  @$pb.TagNumber(10)
  $core.int get unchangedWrong => $_getIZ(9);
  @$pb.TagNumber(10)
  set unchangedWrong($core.int value) => $_setUnsignedInt32(9, value);
  @$pb.TagNumber(10)
  $core.bool hasUnchangedWrong() => $_has(9);
  @$pb.TagNumber(10)
  void clearUnchangedWrong() => $_clearField(10);

  @$pb.TagNumber(11)
  $core.int get unchangedCorrect => $_getIZ(10);
  @$pb.TagNumber(11)
  set unchangedCorrect($core.int value) => $_setUnsignedInt32(10, value);
  @$pb.TagNumber(11)
  $core.bool hasUnchangedCorrect() => $_has(10);
  @$pb.TagNumber(11)
  void clearUnchangedCorrect() => $_clearField(11);

  @$pb.TagNumber(12)
  $core.int get questionCount => $_getIZ(11);
  @$pb.TagNumber(12)
  set questionCount($core.int value) => $_setUnsignedInt32(11, value);
  @$pb.TagNumber(12)
  $core.bool hasQuestionCount() => $_has(11);
  @$pb.TagNumber(12)
  void clearQuestionCount() => $_clearField(12);

  @$pb.TagNumber(13)
  $core.String get sampleClassification => $_getSZ(12);
  @$pb.TagNumber(13)
  set sampleClassification($core.String value) => $_setString(12, value);
  @$pb.TagNumber(13)
  $core.bool hasSampleClassification() => $_has(12);
  @$pb.TagNumber(13)
  void clearSampleClassification() => $_clearField(13);

  @$pb.TagNumber(14)
  $core.bool get focused => $_getBF(13);
  @$pb.TagNumber(14)
  set focused($core.bool value) => $_setBool(13, value);
  @$pb.TagNumber(14)
  $core.bool hasFocused() => $_has(13);
  @$pb.TagNumber(14)
  void clearFocused() => $_clearField(14);

  @$pb.TagNumber(15)
  $core.double get aggregateAccuracy => $_getN(14);
  @$pb.TagNumber(15)
  set aggregateAccuracy($core.double value) => $_setDouble(14, value);
  @$pb.TagNumber(15)
  $core.bool hasAggregateAccuracy() => $_has(14);
  @$pb.TagNumber(15)
  void clearAggregateAccuracy() => $_clearField(15);

  @$pb.TagNumber(16)
  $core.int get aggregateCorrect => $_getIZ(15);
  @$pb.TagNumber(16)
  set aggregateCorrect($core.int value) => $_setUnsignedInt32(15, value);
  @$pb.TagNumber(16)
  $core.bool hasAggregateCorrect() => $_has(15);
  @$pb.TagNumber(16)
  void clearAggregateCorrect() => $_clearField(16);

  @$pb.TagNumber(17)
  $core.int get aggregateTotal => $_getIZ(16);
  @$pb.TagNumber(17)
  set aggregateTotal($core.int value) => $_setUnsignedInt32(16, value);
  @$pb.TagNumber(17)
  $core.bool hasAggregateTotal() => $_has(16);
  @$pb.TagNumber(17)
  void clearAggregateTotal() => $_clearField(17);
}

class RunSummary extends $pb.GeneratedMessage {
  factory RunSummary({
    $core.String? runId,
    $core.String? origin,
    $core.String? system,
    $core.String? benchmark,
    $core.int? limit,
    $core.String? runName,
    $core.String? displayName,
    $core.String? runKind,
    $core.String? registrySection,
    $core.bool? isMetaRecord,
    $core.String? tuningCohort,
    $core.String? tuningShape,
    $core.String? configLabel,
    $core.String? settingsLabel,
    $core.double? accuracy,
    $core.int? accuracyCorrect,
    $core.int? accuracyTotal,
    $core.double? taskAveragedAccuracy,
    $core.double? abstentionAccuracy,
    $fixnum.Int64? costMicroUsd,
    $core.double? latencyMsP50,
    $core.double? latencyMsP95,
    $core.String? configSignature,
    $core.String? cohortId,
    $core.String? datasetFingerprint,
    $core.String? judgeModel,
    $core.String? judgePromptMode,
    $core.bool? oracleGold,
    $core.String? createdAt,
    $core.double? modifiedMs,
    $core.Iterable<$core.MapEntry<$core.String, QTypeScore>>? perQuestionType,
    $core.Iterable<$core.String>? artifactsAvailable,
    $core.Iterable<$core.String>? artifactsMissing,
    $core.bool? nativeStateAvailable,
    $core.bool? isTrialRun,
    $core.Iterable<TrialMarker>? trialMarkers,
  }) {
    final result = create();
    if (runId != null) result.runId = runId;
    if (origin != null) result.origin = origin;
    if (system != null) result.system = system;
    if (benchmark != null) result.benchmark = benchmark;
    if (limit != null) result.limit = limit;
    if (runName != null) result.runName = runName;
    if (displayName != null) result.displayName = displayName;
    if (runKind != null) result.runKind = runKind;
    if (registrySection != null) result.registrySection = registrySection;
    if (isMetaRecord != null) result.isMetaRecord = isMetaRecord;
    if (tuningCohort != null) result.tuningCohort = tuningCohort;
    if (tuningShape != null) result.tuningShape = tuningShape;
    if (configLabel != null) result.configLabel = configLabel;
    if (settingsLabel != null) result.settingsLabel = settingsLabel;
    if (accuracy != null) result.accuracy = accuracy;
    if (accuracyCorrect != null) result.accuracyCorrect = accuracyCorrect;
    if (accuracyTotal != null) result.accuracyTotal = accuracyTotal;
    if (taskAveragedAccuracy != null)
      result.taskAveragedAccuracy = taskAveragedAccuracy;
    if (abstentionAccuracy != null)
      result.abstentionAccuracy = abstentionAccuracy;
    if (costMicroUsd != null) result.costMicroUsd = costMicroUsd;
    if (latencyMsP50 != null) result.latencyMsP50 = latencyMsP50;
    if (latencyMsP95 != null) result.latencyMsP95 = latencyMsP95;
    if (configSignature != null) result.configSignature = configSignature;
    if (cohortId != null) result.cohortId = cohortId;
    if (datasetFingerprint != null)
      result.datasetFingerprint = datasetFingerprint;
    if (judgeModel != null) result.judgeModel = judgeModel;
    if (judgePromptMode != null) result.judgePromptMode = judgePromptMode;
    if (oracleGold != null) result.oracleGold = oracleGold;
    if (createdAt != null) result.createdAt = createdAt;
    if (modifiedMs != null) result.modifiedMs = modifiedMs;
    if (perQuestionType != null)
      result.perQuestionType.addEntries(perQuestionType);
    if (artifactsAvailable != null)
      result.artifactsAvailable.addAll(artifactsAvailable);
    if (artifactsMissing != null)
      result.artifactsMissing.addAll(artifactsMissing);
    if (nativeStateAvailable != null)
      result.nativeStateAvailable = nativeStateAvailable;
    if (isTrialRun != null) result.isTrialRun = isTrialRun;
    if (trialMarkers != null) result.trialMarkers.addAll(trialMarkers);
    return result;
  }

  RunSummary._();

  factory RunSummary.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory RunSummary.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'RunSummary',
      package: const $pb.PackageName(
          _omitMessageNames ? '' : 'membench.dashboard.v1'),
      createEmptyInstance: create)
    ..aOS(1, _omitFieldNames ? '' : 'runId')
    ..aOS(2, _omitFieldNames ? '' : 'origin')
    ..aOS(3, _omitFieldNames ? '' : 'system')
    ..aOS(4, _omitFieldNames ? '' : 'benchmark')
    ..aI(5, _omitFieldNames ? '' : 'limit', fieldType: $pb.PbFieldType.OU3)
    ..aOS(6, _omitFieldNames ? '' : 'runName')
    ..aOS(7, _omitFieldNames ? '' : 'displayName')
    ..aOS(8, _omitFieldNames ? '' : 'runKind')
    ..aOS(9, _omitFieldNames ? '' : 'registrySection')
    ..aOB(10, _omitFieldNames ? '' : 'isMetaRecord')
    ..aOS(11, _omitFieldNames ? '' : 'tuningCohort')
    ..aOS(12, _omitFieldNames ? '' : 'tuningShape')
    ..aOS(13, _omitFieldNames ? '' : 'configLabel')
    ..aOS(14, _omitFieldNames ? '' : 'settingsLabel')
    ..aD(15, _omitFieldNames ? '' : 'accuracy')
    ..aI(16, _omitFieldNames ? '' : 'accuracyCorrect',
        fieldType: $pb.PbFieldType.OU3)
    ..aI(17, _omitFieldNames ? '' : 'accuracyTotal',
        fieldType: $pb.PbFieldType.OU3)
    ..aD(18, _omitFieldNames ? '' : 'taskAveragedAccuracy')
    ..aD(19, _omitFieldNames ? '' : 'abstentionAccuracy')
    ..aInt64(20, _omitFieldNames ? '' : 'costMicroUsd')
    ..aD(21, _omitFieldNames ? '' : 'latencyMsP50')
    ..aD(22, _omitFieldNames ? '' : 'latencyMsP95')
    ..aOS(23, _omitFieldNames ? '' : 'configSignature')
    ..aOS(24, _omitFieldNames ? '' : 'cohortId')
    ..aOS(25, _omitFieldNames ? '' : 'datasetFingerprint')
    ..aOS(26, _omitFieldNames ? '' : 'judgeModel')
    ..aOS(27, _omitFieldNames ? '' : 'judgePromptMode')
    ..aOB(28, _omitFieldNames ? '' : 'oracleGold')
    ..aOS(29, _omitFieldNames ? '' : 'createdAt')
    ..aD(30, _omitFieldNames ? '' : 'modifiedMs')
    ..m<$core.String, QTypeScore>(31, _omitFieldNames ? '' : 'perQuestionType',
        entryClassName: 'RunSummary.PerQuestionTypeEntry',
        keyFieldType: $pb.PbFieldType.OS,
        valueFieldType: $pb.PbFieldType.OM,
        valueCreator: QTypeScore.create,
        valueDefaultOrMaker: QTypeScore.getDefault,
        packageName: const $pb.PackageName('membench.dashboard.v1'))
    ..pPS(32, _omitFieldNames ? '' : 'artifactsAvailable')
    ..pPS(33, _omitFieldNames ? '' : 'artifactsMissing')
    ..aOB(34, _omitFieldNames ? '' : 'nativeStateAvailable')
    ..aOB(35, _omitFieldNames ? '' : 'isTrialRun')
    ..pPM<TrialMarker>(36, _omitFieldNames ? '' : 'trialMarkers',
        subBuilder: TrialMarker.create)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  RunSummary clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  RunSummary copyWith(void Function(RunSummary) updates) =>
      super.copyWith((message) => updates(message as RunSummary)) as RunSummary;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static RunSummary create() => RunSummary._();
  @$core.override
  RunSummary createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static RunSummary getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<RunSummary>(create);
  static RunSummary? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get runId => $_getSZ(0);
  @$pb.TagNumber(1)
  set runId($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasRunId() => $_has(0);
  @$pb.TagNumber(1)
  void clearRunId() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get origin => $_getSZ(1);
  @$pb.TagNumber(2)
  set origin($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasOrigin() => $_has(1);
  @$pb.TagNumber(2)
  void clearOrigin() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.String get system => $_getSZ(2);
  @$pb.TagNumber(3)
  set system($core.String value) => $_setString(2, value);
  @$pb.TagNumber(3)
  $core.bool hasSystem() => $_has(2);
  @$pb.TagNumber(3)
  void clearSystem() => $_clearField(3);

  @$pb.TagNumber(4)
  $core.String get benchmark => $_getSZ(3);
  @$pb.TagNumber(4)
  set benchmark($core.String value) => $_setString(3, value);
  @$pb.TagNumber(4)
  $core.bool hasBenchmark() => $_has(3);
  @$pb.TagNumber(4)
  void clearBenchmark() => $_clearField(4);

  @$pb.TagNumber(5)
  $core.int get limit => $_getIZ(4);
  @$pb.TagNumber(5)
  set limit($core.int value) => $_setUnsignedInt32(4, value);
  @$pb.TagNumber(5)
  $core.bool hasLimit() => $_has(4);
  @$pb.TagNumber(5)
  void clearLimit() => $_clearField(5);

  @$pb.TagNumber(6)
  $core.String get runName => $_getSZ(5);
  @$pb.TagNumber(6)
  set runName($core.String value) => $_setString(5, value);
  @$pb.TagNumber(6)
  $core.bool hasRunName() => $_has(5);
  @$pb.TagNumber(6)
  void clearRunName() => $_clearField(6);

  @$pb.TagNumber(7)
  $core.String get displayName => $_getSZ(6);
  @$pb.TagNumber(7)
  set displayName($core.String value) => $_setString(6, value);
  @$pb.TagNumber(7)
  $core.bool hasDisplayName() => $_has(6);
  @$pb.TagNumber(7)
  void clearDisplayName() => $_clearField(7);

  @$pb.TagNumber(8)
  $core.String get runKind => $_getSZ(7);
  @$pb.TagNumber(8)
  set runKind($core.String value) => $_setString(7, value);
  @$pb.TagNumber(8)
  $core.bool hasRunKind() => $_has(7);
  @$pb.TagNumber(8)
  void clearRunKind() => $_clearField(8);

  @$pb.TagNumber(9)
  $core.String get registrySection => $_getSZ(8);
  @$pb.TagNumber(9)
  set registrySection($core.String value) => $_setString(8, value);
  @$pb.TagNumber(9)
  $core.bool hasRegistrySection() => $_has(8);
  @$pb.TagNumber(9)
  void clearRegistrySection() => $_clearField(9);

  @$pb.TagNumber(10)
  $core.bool get isMetaRecord => $_getBF(9);
  @$pb.TagNumber(10)
  set isMetaRecord($core.bool value) => $_setBool(9, value);
  @$pb.TagNumber(10)
  $core.bool hasIsMetaRecord() => $_has(9);
  @$pb.TagNumber(10)
  void clearIsMetaRecord() => $_clearField(10);

  @$pb.TagNumber(11)
  $core.String get tuningCohort => $_getSZ(10);
  @$pb.TagNumber(11)
  set tuningCohort($core.String value) => $_setString(10, value);
  @$pb.TagNumber(11)
  $core.bool hasTuningCohort() => $_has(10);
  @$pb.TagNumber(11)
  void clearTuningCohort() => $_clearField(11);

  @$pb.TagNumber(12)
  $core.String get tuningShape => $_getSZ(11);
  @$pb.TagNumber(12)
  set tuningShape($core.String value) => $_setString(11, value);
  @$pb.TagNumber(12)
  $core.bool hasTuningShape() => $_has(11);
  @$pb.TagNumber(12)
  void clearTuningShape() => $_clearField(12);

  @$pb.TagNumber(13)
  $core.String get configLabel => $_getSZ(12);
  @$pb.TagNumber(13)
  set configLabel($core.String value) => $_setString(12, value);
  @$pb.TagNumber(13)
  $core.bool hasConfigLabel() => $_has(12);
  @$pb.TagNumber(13)
  void clearConfigLabel() => $_clearField(13);

  @$pb.TagNumber(14)
  $core.String get settingsLabel => $_getSZ(13);
  @$pb.TagNumber(14)
  set settingsLabel($core.String value) => $_setString(13, value);
  @$pb.TagNumber(14)
  $core.bool hasSettingsLabel() => $_has(13);
  @$pb.TagNumber(14)
  void clearSettingsLabel() => $_clearField(14);

  @$pb.TagNumber(15)
  $core.double get accuracy => $_getN(14);
  @$pb.TagNumber(15)
  set accuracy($core.double value) => $_setDouble(14, value);
  @$pb.TagNumber(15)
  $core.bool hasAccuracy() => $_has(14);
  @$pb.TagNumber(15)
  void clearAccuracy() => $_clearField(15);

  @$pb.TagNumber(16)
  $core.int get accuracyCorrect => $_getIZ(15);
  @$pb.TagNumber(16)
  set accuracyCorrect($core.int value) => $_setUnsignedInt32(15, value);
  @$pb.TagNumber(16)
  $core.bool hasAccuracyCorrect() => $_has(15);
  @$pb.TagNumber(16)
  void clearAccuracyCorrect() => $_clearField(16);

  @$pb.TagNumber(17)
  $core.int get accuracyTotal => $_getIZ(16);
  @$pb.TagNumber(17)
  set accuracyTotal($core.int value) => $_setUnsignedInt32(16, value);
  @$pb.TagNumber(17)
  $core.bool hasAccuracyTotal() => $_has(16);
  @$pb.TagNumber(17)
  void clearAccuracyTotal() => $_clearField(17);

  @$pb.TagNumber(18)
  $core.double get taskAveragedAccuracy => $_getN(17);
  @$pb.TagNumber(18)
  set taskAveragedAccuracy($core.double value) => $_setDouble(17, value);
  @$pb.TagNumber(18)
  $core.bool hasTaskAveragedAccuracy() => $_has(17);
  @$pb.TagNumber(18)
  void clearTaskAveragedAccuracy() => $_clearField(18);

  @$pb.TagNumber(19)
  $core.double get abstentionAccuracy => $_getN(18);
  @$pb.TagNumber(19)
  set abstentionAccuracy($core.double value) => $_setDouble(18, value);
  @$pb.TagNumber(19)
  $core.bool hasAbstentionAccuracy() => $_has(18);
  @$pb.TagNumber(19)
  void clearAbstentionAccuracy() => $_clearField(19);

  @$pb.TagNumber(20)
  $fixnum.Int64 get costMicroUsd => $_getI64(19);
  @$pb.TagNumber(20)
  set costMicroUsd($fixnum.Int64 value) => $_setInt64(19, value);
  @$pb.TagNumber(20)
  $core.bool hasCostMicroUsd() => $_has(19);
  @$pb.TagNumber(20)
  void clearCostMicroUsd() => $_clearField(20);

  @$pb.TagNumber(21)
  $core.double get latencyMsP50 => $_getN(20);
  @$pb.TagNumber(21)
  set latencyMsP50($core.double value) => $_setDouble(20, value);
  @$pb.TagNumber(21)
  $core.bool hasLatencyMsP50() => $_has(20);
  @$pb.TagNumber(21)
  void clearLatencyMsP50() => $_clearField(21);

  @$pb.TagNumber(22)
  $core.double get latencyMsP95 => $_getN(21);
  @$pb.TagNumber(22)
  set latencyMsP95($core.double value) => $_setDouble(21, value);
  @$pb.TagNumber(22)
  $core.bool hasLatencyMsP95() => $_has(21);
  @$pb.TagNumber(22)
  void clearLatencyMsP95() => $_clearField(22);

  @$pb.TagNumber(23)
  $core.String get configSignature => $_getSZ(22);
  @$pb.TagNumber(23)
  set configSignature($core.String value) => $_setString(22, value);
  @$pb.TagNumber(23)
  $core.bool hasConfigSignature() => $_has(22);
  @$pb.TagNumber(23)
  void clearConfigSignature() => $_clearField(23);

  @$pb.TagNumber(24)
  $core.String get cohortId => $_getSZ(23);
  @$pb.TagNumber(24)
  set cohortId($core.String value) => $_setString(23, value);
  @$pb.TagNumber(24)
  $core.bool hasCohortId() => $_has(23);
  @$pb.TagNumber(24)
  void clearCohortId() => $_clearField(24);

  @$pb.TagNumber(25)
  $core.String get datasetFingerprint => $_getSZ(24);
  @$pb.TagNumber(25)
  set datasetFingerprint($core.String value) => $_setString(24, value);
  @$pb.TagNumber(25)
  $core.bool hasDatasetFingerprint() => $_has(24);
  @$pb.TagNumber(25)
  void clearDatasetFingerprint() => $_clearField(25);

  @$pb.TagNumber(26)
  $core.String get judgeModel => $_getSZ(25);
  @$pb.TagNumber(26)
  set judgeModel($core.String value) => $_setString(25, value);
  @$pb.TagNumber(26)
  $core.bool hasJudgeModel() => $_has(25);
  @$pb.TagNumber(26)
  void clearJudgeModel() => $_clearField(26);

  @$pb.TagNumber(27)
  $core.String get judgePromptMode => $_getSZ(26);
  @$pb.TagNumber(27)
  set judgePromptMode($core.String value) => $_setString(26, value);
  @$pb.TagNumber(27)
  $core.bool hasJudgePromptMode() => $_has(26);
  @$pb.TagNumber(27)
  void clearJudgePromptMode() => $_clearField(27);

  @$pb.TagNumber(28)
  $core.bool get oracleGold => $_getBF(27);
  @$pb.TagNumber(28)
  set oracleGold($core.bool value) => $_setBool(27, value);
  @$pb.TagNumber(28)
  $core.bool hasOracleGold() => $_has(27);
  @$pb.TagNumber(28)
  void clearOracleGold() => $_clearField(28);

  @$pb.TagNumber(29)
  $core.String get createdAt => $_getSZ(28);
  @$pb.TagNumber(29)
  set createdAt($core.String value) => $_setString(28, value);
  @$pb.TagNumber(29)
  $core.bool hasCreatedAt() => $_has(28);
  @$pb.TagNumber(29)
  void clearCreatedAt() => $_clearField(29);

  @$pb.TagNumber(30)
  $core.double get modifiedMs => $_getN(29);
  @$pb.TagNumber(30)
  set modifiedMs($core.double value) => $_setDouble(29, value);
  @$pb.TagNumber(30)
  $core.bool hasModifiedMs() => $_has(29);
  @$pb.TagNumber(30)
  void clearModifiedMs() => $_clearField(30);

  @$pb.TagNumber(31)
  $pb.PbMap<$core.String, QTypeScore> get perQuestionType => $_getMap(30);

  @$pb.TagNumber(32)
  $pb.PbList<$core.String> get artifactsAvailable => $_getList(31);

  @$pb.TagNumber(33)
  $pb.PbList<$core.String> get artifactsMissing => $_getList(32);

  @$pb.TagNumber(34)
  $core.bool get nativeStateAvailable => $_getBF(33);
  @$pb.TagNumber(34)
  set nativeStateAvailable($core.bool value) => $_setBool(33, value);
  @$pb.TagNumber(34)
  $core.bool hasNativeStateAvailable() => $_has(33);
  @$pb.TagNumber(34)
  void clearNativeStateAvailable() => $_clearField(34);

  @$pb.TagNumber(35)
  $core.bool get isTrialRun => $_getBF(34);
  @$pb.TagNumber(35)
  set isTrialRun($core.bool value) => $_setBool(34, value);
  @$pb.TagNumber(35)
  $core.bool hasIsTrialRun() => $_has(34);
  @$pb.TagNumber(35)
  void clearIsTrialRun() => $_clearField(35);

  @$pb.TagNumber(36)
  $pb.PbList<TrialMarker> get trialMarkers => $_getList(35);
}

class RunsResponse extends $pb.GeneratedMessage {
  factory RunsResponse({
    $core.Iterable<RunSummary>? runs,
  }) {
    final result = create();
    if (runs != null) result.runs.addAll(runs);
    return result;
  }

  RunsResponse._();

  factory RunsResponse.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory RunsResponse.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'RunsResponse',
      package: const $pb.PackageName(
          _omitMessageNames ? '' : 'membench.dashboard.v1'),
      createEmptyInstance: create)
    ..pPM<RunSummary>(1, _omitFieldNames ? '' : 'runs',
        subBuilder: RunSummary.create)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  RunsResponse clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  RunsResponse copyWith(void Function(RunsResponse) updates) =>
      super.copyWith((message) => updates(message as RunsResponse))
          as RunsResponse;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static RunsResponse create() => RunsResponse._();
  @$core.override
  RunsResponse createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static RunsResponse getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<RunsResponse>(create);
  static RunsResponse? _defaultInstance;

  @$pb.TagNumber(1)
  $pb.PbList<RunSummary> get runs => $_getList(0);
}

class PendingRun extends $pb.GeneratedMessage {
  factory PendingRun({
    $core.double? ageSecs,
    $core.String? benchmark,
    $core.String? configLabel,
    $core.int? hypotheses,
    $core.int? ingested,
    $core.int? limit,
    $core.bool? oracleGold,
    $core.String? origin,
    $core.String? runId,
    $core.String? runName,
    $core.String? settingsLabel,
    $core.double? startedMs,
    $core.String? status,
    $core.String? system,
    $core.double? updatedMs,
  }) {
    final result = create();
    if (ageSecs != null) result.ageSecs = ageSecs;
    if (benchmark != null) result.benchmark = benchmark;
    if (configLabel != null) result.configLabel = configLabel;
    if (hypotheses != null) result.hypotheses = hypotheses;
    if (ingested != null) result.ingested = ingested;
    if (limit != null) result.limit = limit;
    if (oracleGold != null) result.oracleGold = oracleGold;
    if (origin != null) result.origin = origin;
    if (runId != null) result.runId = runId;
    if (runName != null) result.runName = runName;
    if (settingsLabel != null) result.settingsLabel = settingsLabel;
    if (startedMs != null) result.startedMs = startedMs;
    if (status != null) result.status = status;
    if (system != null) result.system = system;
    if (updatedMs != null) result.updatedMs = updatedMs;
    return result;
  }

  PendingRun._();

  factory PendingRun.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory PendingRun.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'PendingRun',
      package: const $pb.PackageName(
          _omitMessageNames ? '' : 'membench.dashboard.v1'),
      createEmptyInstance: create)
    ..aD(1, _omitFieldNames ? '' : 'ageSecs')
    ..aOS(2, _omitFieldNames ? '' : 'benchmark')
    ..aOS(3, _omitFieldNames ? '' : 'configLabel')
    ..aI(4, _omitFieldNames ? '' : 'hypotheses', fieldType: $pb.PbFieldType.OU3)
    ..aI(5, _omitFieldNames ? '' : 'ingested', fieldType: $pb.PbFieldType.OU3)
    ..aI(6, _omitFieldNames ? '' : 'limit', fieldType: $pb.PbFieldType.OU3)
    ..aOB(7, _omitFieldNames ? '' : 'oracleGold')
    ..aOS(8, _omitFieldNames ? '' : 'origin')
    ..aOS(9, _omitFieldNames ? '' : 'runId')
    ..aOS(10, _omitFieldNames ? '' : 'runName')
    ..aOS(11, _omitFieldNames ? '' : 'settingsLabel')
    ..aD(12, _omitFieldNames ? '' : 'startedMs')
    ..aOS(13, _omitFieldNames ? '' : 'status')
    ..aOS(14, _omitFieldNames ? '' : 'system')
    ..aD(15, _omitFieldNames ? '' : 'updatedMs')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  PendingRun clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  PendingRun copyWith(void Function(PendingRun) updates) =>
      super.copyWith((message) => updates(message as PendingRun)) as PendingRun;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static PendingRun create() => PendingRun._();
  @$core.override
  PendingRun createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static PendingRun getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<PendingRun>(create);
  static PendingRun? _defaultInstance;

  @$pb.TagNumber(1)
  $core.double get ageSecs => $_getN(0);
  @$pb.TagNumber(1)
  set ageSecs($core.double value) => $_setDouble(0, value);
  @$pb.TagNumber(1)
  $core.bool hasAgeSecs() => $_has(0);
  @$pb.TagNumber(1)
  void clearAgeSecs() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get benchmark => $_getSZ(1);
  @$pb.TagNumber(2)
  set benchmark($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasBenchmark() => $_has(1);
  @$pb.TagNumber(2)
  void clearBenchmark() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.String get configLabel => $_getSZ(2);
  @$pb.TagNumber(3)
  set configLabel($core.String value) => $_setString(2, value);
  @$pb.TagNumber(3)
  $core.bool hasConfigLabel() => $_has(2);
  @$pb.TagNumber(3)
  void clearConfigLabel() => $_clearField(3);

  @$pb.TagNumber(4)
  $core.int get hypotheses => $_getIZ(3);
  @$pb.TagNumber(4)
  set hypotheses($core.int value) => $_setUnsignedInt32(3, value);
  @$pb.TagNumber(4)
  $core.bool hasHypotheses() => $_has(3);
  @$pb.TagNumber(4)
  void clearHypotheses() => $_clearField(4);

  @$pb.TagNumber(5)
  $core.int get ingested => $_getIZ(4);
  @$pb.TagNumber(5)
  set ingested($core.int value) => $_setUnsignedInt32(4, value);
  @$pb.TagNumber(5)
  $core.bool hasIngested() => $_has(4);
  @$pb.TagNumber(5)
  void clearIngested() => $_clearField(5);

  @$pb.TagNumber(6)
  $core.int get limit => $_getIZ(5);
  @$pb.TagNumber(6)
  set limit($core.int value) => $_setUnsignedInt32(5, value);
  @$pb.TagNumber(6)
  $core.bool hasLimit() => $_has(5);
  @$pb.TagNumber(6)
  void clearLimit() => $_clearField(6);

  @$pb.TagNumber(7)
  $core.bool get oracleGold => $_getBF(6);
  @$pb.TagNumber(7)
  set oracleGold($core.bool value) => $_setBool(6, value);
  @$pb.TagNumber(7)
  $core.bool hasOracleGold() => $_has(6);
  @$pb.TagNumber(7)
  void clearOracleGold() => $_clearField(7);

  @$pb.TagNumber(8)
  $core.String get origin => $_getSZ(7);
  @$pb.TagNumber(8)
  set origin($core.String value) => $_setString(7, value);
  @$pb.TagNumber(8)
  $core.bool hasOrigin() => $_has(7);
  @$pb.TagNumber(8)
  void clearOrigin() => $_clearField(8);

  @$pb.TagNumber(9)
  $core.String get runId => $_getSZ(8);
  @$pb.TagNumber(9)
  set runId($core.String value) => $_setString(8, value);
  @$pb.TagNumber(9)
  $core.bool hasRunId() => $_has(8);
  @$pb.TagNumber(9)
  void clearRunId() => $_clearField(9);

  @$pb.TagNumber(10)
  $core.String get runName => $_getSZ(9);
  @$pb.TagNumber(10)
  set runName($core.String value) => $_setString(9, value);
  @$pb.TagNumber(10)
  $core.bool hasRunName() => $_has(9);
  @$pb.TagNumber(10)
  void clearRunName() => $_clearField(10);

  @$pb.TagNumber(11)
  $core.String get settingsLabel => $_getSZ(10);
  @$pb.TagNumber(11)
  set settingsLabel($core.String value) => $_setString(10, value);
  @$pb.TagNumber(11)
  $core.bool hasSettingsLabel() => $_has(10);
  @$pb.TagNumber(11)
  void clearSettingsLabel() => $_clearField(11);

  @$pb.TagNumber(12)
  $core.double get startedMs => $_getN(11);
  @$pb.TagNumber(12)
  set startedMs($core.double value) => $_setDouble(11, value);
  @$pb.TagNumber(12)
  $core.bool hasStartedMs() => $_has(11);
  @$pb.TagNumber(12)
  void clearStartedMs() => $_clearField(12);

  @$pb.TagNumber(13)
  $core.String get status => $_getSZ(12);
  @$pb.TagNumber(13)
  set status($core.String value) => $_setString(12, value);
  @$pb.TagNumber(13)
  $core.bool hasStatus() => $_has(12);
  @$pb.TagNumber(13)
  void clearStatus() => $_clearField(13);

  @$pb.TagNumber(14)
  $core.String get system => $_getSZ(13);
  @$pb.TagNumber(14)
  set system($core.String value) => $_setString(13, value);
  @$pb.TagNumber(14)
  $core.bool hasSystem() => $_has(13);
  @$pb.TagNumber(14)
  void clearSystem() => $_clearField(14);

  @$pb.TagNumber(15)
  $core.double get updatedMs => $_getN(14);
  @$pb.TagNumber(15)
  set updatedMs($core.double value) => $_setDouble(14, value);
  @$pb.TagNumber(15)
  $core.bool hasUpdatedMs() => $_has(14);
  @$pb.TagNumber(15)
  void clearUpdatedMs() => $_clearField(15);
}

class PendingResponse extends $pb.GeneratedMessage {
  factory PendingResponse({
    $core.Iterable<PendingRun>? pending,
  }) {
    final result = create();
    if (pending != null) result.pending.addAll(pending);
    return result;
  }

  PendingResponse._();

  factory PendingResponse.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory PendingResponse.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'PendingResponse',
      package: const $pb.PackageName(
          _omitMessageNames ? '' : 'membench.dashboard.v1'),
      createEmptyInstance: create)
    ..pPM<PendingRun>(1, _omitFieldNames ? '' : 'pending',
        subBuilder: PendingRun.create)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  PendingResponse clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  PendingResponse copyWith(void Function(PendingResponse) updates) =>
      super.copyWith((message) => updates(message as PendingResponse))
          as PendingResponse;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static PendingResponse create() => PendingResponse._();
  @$core.override
  PendingResponse createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static PendingResponse getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<PendingResponse>(create);
  static PendingResponse? _defaultInstance;

  @$pb.TagNumber(1)
  $pb.PbList<PendingRun> get pending => $_getList(0);
}

class QuestionRow extends $pb.GeneratedMessage {
  factory QuestionRow({
    $core.String? questionId,
    $core.String? questionType,
    $core.String? question,
    $core.String? goldAnswer,
    $core.String? hypothesis,
    $core.bool? label,
    $core.bool? isAbstention,
    $core.String? judgeRaw,
    $core.String? judgeSystemPrompt,
    $core.String? judgeUserPrompt,
    $core.String? judgeModel,
    $core.String? routerPick,
    $core.String? initialPick,
    $core.String? finalPick,
    $core.String? debugArtifact,
    $core.String? error,
  }) {
    final result = create();
    if (questionId != null) result.questionId = questionId;
    if (questionType != null) result.questionType = questionType;
    if (question != null) result.question = question;
    if (goldAnswer != null) result.goldAnswer = goldAnswer;
    if (hypothesis != null) result.hypothesis = hypothesis;
    if (label != null) result.label = label;
    if (isAbstention != null) result.isAbstention = isAbstention;
    if (judgeRaw != null) result.judgeRaw = judgeRaw;
    if (judgeSystemPrompt != null) result.judgeSystemPrompt = judgeSystemPrompt;
    if (judgeUserPrompt != null) result.judgeUserPrompt = judgeUserPrompt;
    if (judgeModel != null) result.judgeModel = judgeModel;
    if (routerPick != null) result.routerPick = routerPick;
    if (initialPick != null) result.initialPick = initialPick;
    if (finalPick != null) result.finalPick = finalPick;
    if (debugArtifact != null) result.debugArtifact = debugArtifact;
    if (error != null) result.error = error;
    return result;
  }

  QuestionRow._();

  factory QuestionRow.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory QuestionRow.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'QuestionRow',
      package: const $pb.PackageName(
          _omitMessageNames ? '' : 'membench.dashboard.v1'),
      createEmptyInstance: create)
    ..aOS(1, _omitFieldNames ? '' : 'questionId')
    ..aOS(2, _omitFieldNames ? '' : 'questionType')
    ..aOS(3, _omitFieldNames ? '' : 'question')
    ..aOS(4, _omitFieldNames ? '' : 'goldAnswer')
    ..aOS(5, _omitFieldNames ? '' : 'hypothesis')
    ..aOB(6, _omitFieldNames ? '' : 'label')
    ..aOB(7, _omitFieldNames ? '' : 'isAbstention')
    ..aOS(8, _omitFieldNames ? '' : 'judgeRaw')
    ..aOS(9, _omitFieldNames ? '' : 'judgeSystemPrompt')
    ..aOS(10, _omitFieldNames ? '' : 'judgeUserPrompt')
    ..aOS(11, _omitFieldNames ? '' : 'judgeModel')
    ..aOS(12, _omitFieldNames ? '' : 'routerPick')
    ..aOS(13, _omitFieldNames ? '' : 'initialPick')
    ..aOS(14, _omitFieldNames ? '' : 'finalPick')
    ..aOS(15, _omitFieldNames ? '' : 'debugArtifact')
    ..aOS(16, _omitFieldNames ? '' : 'error')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  QuestionRow clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  QuestionRow copyWith(void Function(QuestionRow) updates) =>
      super.copyWith((message) => updates(message as QuestionRow))
          as QuestionRow;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static QuestionRow create() => QuestionRow._();
  @$core.override
  QuestionRow createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static QuestionRow getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<QuestionRow>(create);
  static QuestionRow? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get questionId => $_getSZ(0);
  @$pb.TagNumber(1)
  set questionId($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasQuestionId() => $_has(0);
  @$pb.TagNumber(1)
  void clearQuestionId() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get questionType => $_getSZ(1);
  @$pb.TagNumber(2)
  set questionType($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasQuestionType() => $_has(1);
  @$pb.TagNumber(2)
  void clearQuestionType() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.String get question => $_getSZ(2);
  @$pb.TagNumber(3)
  set question($core.String value) => $_setString(2, value);
  @$pb.TagNumber(3)
  $core.bool hasQuestion() => $_has(2);
  @$pb.TagNumber(3)
  void clearQuestion() => $_clearField(3);

  @$pb.TagNumber(4)
  $core.String get goldAnswer => $_getSZ(3);
  @$pb.TagNumber(4)
  set goldAnswer($core.String value) => $_setString(3, value);
  @$pb.TagNumber(4)
  $core.bool hasGoldAnswer() => $_has(3);
  @$pb.TagNumber(4)
  void clearGoldAnswer() => $_clearField(4);

  @$pb.TagNumber(5)
  $core.String get hypothesis => $_getSZ(4);
  @$pb.TagNumber(5)
  set hypothesis($core.String value) => $_setString(4, value);
  @$pb.TagNumber(5)
  $core.bool hasHypothesis() => $_has(4);
  @$pb.TagNumber(5)
  void clearHypothesis() => $_clearField(5);

  @$pb.TagNumber(6)
  $core.bool get label => $_getBF(5);
  @$pb.TagNumber(6)
  set label($core.bool value) => $_setBool(5, value);
  @$pb.TagNumber(6)
  $core.bool hasLabel() => $_has(5);
  @$pb.TagNumber(6)
  void clearLabel() => $_clearField(6);

  @$pb.TagNumber(7)
  $core.bool get isAbstention => $_getBF(6);
  @$pb.TagNumber(7)
  set isAbstention($core.bool value) => $_setBool(6, value);
  @$pb.TagNumber(7)
  $core.bool hasIsAbstention() => $_has(6);
  @$pb.TagNumber(7)
  void clearIsAbstention() => $_clearField(7);

  @$pb.TagNumber(8)
  $core.String get judgeRaw => $_getSZ(7);
  @$pb.TagNumber(8)
  set judgeRaw($core.String value) => $_setString(7, value);
  @$pb.TagNumber(8)
  $core.bool hasJudgeRaw() => $_has(7);
  @$pb.TagNumber(8)
  void clearJudgeRaw() => $_clearField(8);

  @$pb.TagNumber(9)
  $core.String get judgeSystemPrompt => $_getSZ(8);
  @$pb.TagNumber(9)
  set judgeSystemPrompt($core.String value) => $_setString(8, value);
  @$pb.TagNumber(9)
  $core.bool hasJudgeSystemPrompt() => $_has(8);
  @$pb.TagNumber(9)
  void clearJudgeSystemPrompt() => $_clearField(9);

  @$pb.TagNumber(10)
  $core.String get judgeUserPrompt => $_getSZ(9);
  @$pb.TagNumber(10)
  set judgeUserPrompt($core.String value) => $_setString(9, value);
  @$pb.TagNumber(10)
  $core.bool hasJudgeUserPrompt() => $_has(9);
  @$pb.TagNumber(10)
  void clearJudgeUserPrompt() => $_clearField(10);

  @$pb.TagNumber(11)
  $core.String get judgeModel => $_getSZ(10);
  @$pb.TagNumber(11)
  set judgeModel($core.String value) => $_setString(10, value);
  @$pb.TagNumber(11)
  $core.bool hasJudgeModel() => $_has(10);
  @$pb.TagNumber(11)
  void clearJudgeModel() => $_clearField(11);

  @$pb.TagNumber(12)
  $core.String get routerPick => $_getSZ(11);
  @$pb.TagNumber(12)
  set routerPick($core.String value) => $_setString(11, value);
  @$pb.TagNumber(12)
  $core.bool hasRouterPick() => $_has(11);
  @$pb.TagNumber(12)
  void clearRouterPick() => $_clearField(12);

  @$pb.TagNumber(13)
  $core.String get initialPick => $_getSZ(12);
  @$pb.TagNumber(13)
  set initialPick($core.String value) => $_setString(12, value);
  @$pb.TagNumber(13)
  $core.bool hasInitialPick() => $_has(12);
  @$pb.TagNumber(13)
  void clearInitialPick() => $_clearField(13);

  @$pb.TagNumber(14)
  $core.String get finalPick => $_getSZ(13);
  @$pb.TagNumber(14)
  set finalPick($core.String value) => $_setString(13, value);
  @$pb.TagNumber(14)
  $core.bool hasFinalPick() => $_has(13);
  @$pb.TagNumber(14)
  void clearFinalPick() => $_clearField(14);

  @$pb.TagNumber(15)
  $core.String get debugArtifact => $_getSZ(14);
  @$pb.TagNumber(15)
  set debugArtifact($core.String value) => $_setString(14, value);
  @$pb.TagNumber(15)
  $core.bool hasDebugArtifact() => $_has(14);
  @$pb.TagNumber(15)
  void clearDebugArtifact() => $_clearField(15);

  @$pb.TagNumber(16)
  $core.String get error => $_getSZ(15);
  @$pb.TagNumber(16)
  set error($core.String value) => $_setString(15, value);
  @$pb.TagNumber(16)
  $core.bool hasError() => $_has(15);
  @$pb.TagNumber(16)
  void clearError() => $_clearField(16);
}

class QuestionsResponse extends $pb.GeneratedMessage {
  factory QuestionsResponse({
    $core.int? total,
    $core.Iterable<QuestionRow>? questions,
  }) {
    final result = create();
    if (total != null) result.total = total;
    if (questions != null) result.questions.addAll(questions);
    return result;
  }

  QuestionsResponse._();

  factory QuestionsResponse.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory QuestionsResponse.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'QuestionsResponse',
      package: const $pb.PackageName(
          _omitMessageNames ? '' : 'membench.dashboard.v1'),
      createEmptyInstance: create)
    ..aI(1, _omitFieldNames ? '' : 'total', fieldType: $pb.PbFieldType.OU3)
    ..pPM<QuestionRow>(2, _omitFieldNames ? '' : 'questions',
        subBuilder: QuestionRow.create)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  QuestionsResponse clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  QuestionsResponse copyWith(void Function(QuestionsResponse) updates) =>
      super.copyWith((message) => updates(message as QuestionsResponse))
          as QuestionsResponse;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static QuestionsResponse create() => QuestionsResponse._();
  @$core.override
  QuestionsResponse createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static QuestionsResponse getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<QuestionsResponse>(create);
  static QuestionsResponse? _defaultInstance;

  @$pb.TagNumber(1)
  $core.int get total => $_getIZ(0);
  @$pb.TagNumber(1)
  set total($core.int value) => $_setUnsignedInt32(0, value);
  @$pb.TagNumber(1)
  $core.bool hasTotal() => $_has(0);
  @$pb.TagNumber(1)
  void clearTotal() => $_clearField(1);

  @$pb.TagNumber(2)
  $pb.PbList<QuestionRow> get questions => $_getList(1);
}

const $core.bool _omitFieldNames =
    $core.bool.fromEnvironment('protobuf.omit_field_names');
const $core.bool _omitMessageNames =
    $core.bool.fromEnvironment('protobuf.omit_message_names');
