// This is a generated file - do not edit.
//
// Generated from membench/dashboard/v1/debugger.proto.

// @dart = 3.3

// ignore_for_file: annotate_overrides, camel_case_types, comment_references
// ignore_for_file: constant_identifier_names
// ignore_for_file: curly_braces_in_flow_control_structures
// ignore_for_file: deprecated_member_use_from_same_package, library_prefixes
// ignore_for_file: non_constant_identifier_names, prefer_relative_imports
// ignore_for_file: unused_import

import 'dart:convert' as $convert;
import 'dart:core' as $core;
import 'dart:typed_data' as $typed_data;

@$core.Deprecated('Use healthResponseDescriptor instead')
const HealthResponse$json = {
  '1': 'HealthResponse',
  '2': [
    {'1': 'ok', '3': 1, '4': 1, '5': 8, '10': 'ok'},
    {'1': 'service', '3': 2, '4': 1, '5': 9, '10': 'service'},
    {'1': 'version', '3': 3, '4': 1, '5': 9, '10': 'version'},
    {'1': 'git_sha', '3': 4, '4': 1, '5': 9, '10': 'gitSha'},
    {'1': 'binary_sha', '3': 5, '4': 1, '5': 9, '10': 'binarySha'},
  ],
};

/// Descriptor for `HealthResponse`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List healthResponseDescriptor = $convert.base64Decode(
    'Cg5IZWFsdGhSZXNwb25zZRIOCgJvaxgBIAEoCFICb2sSGAoHc2VydmljZRgCIAEoCVIHc2Vydm'
    'ljZRIYCgd2ZXJzaW9uGAMgASgJUgd2ZXJzaW9uEhcKB2dpdF9zaGEYBCABKAlSBmdpdFNoYRId'
    'CgpiaW5hcnlfc2hhGAUgASgJUgliaW5hcnlTaGE=');

@$core.Deprecated('Use qTypeScoreDescriptor instead')
const QTypeScore$json = {
  '1': 'QTypeScore',
  '2': [
    {'1': 'accuracy', '3': 1, '4': 1, '5': 1, '10': 'accuracy'},
    {'1': 'n', '3': 2, '4': 1, '5': 13, '10': 'n'},
    {'1': 'correct', '3': 3, '4': 1, '5': 13, '10': 'correct'},
    {'1': 'total', '3': 4, '4': 1, '5': 13, '10': 'total'},
  ],
};

/// Descriptor for `QTypeScore`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List qTypeScoreDescriptor = $convert.base64Decode(
    'CgpRVHlwZVNjb3JlEhoKCGFjY3VyYWN5GAEgASgBUghhY2N1cmFjeRIMCgFuGAIgASgNUgFuEh'
    'gKB2NvcnJlY3QYAyABKA1SB2NvcnJlY3QSFAoFdG90YWwYBCABKA1SBXRvdGFs');

@$core.Deprecated('Use trialMarkerDescriptor instead')
const TrialMarker$json = {
  '1': 'TrialMarker',
  '2': [
    {'1': 'stack_id', '3': 1, '4': 1, '5': 9, '10': 'stackId'},
    {'1': 'change_id', '3': 2, '4': 1, '5': 9, '10': 'changeId'},
    {'1': 'change_title', '3': 3, '4': 1, '5': 9, '10': 'changeTitle'},
    {'1': 'decision', '3': 4, '4': 1, '5': 9, '10': 'decision'},
    {'1': 'analysis_path', '3': 5, '4': 1, '5': 9, '10': 'analysisPath'},
    {
      '1': 'compared_to_run_id',
      '3': 6,
      '4': 1,
      '5': 9,
      '9': 0,
      '10': 'comparedToRunId',
      '17': true
    },
    {
      '1': 'original_baseline_run_id',
      '3': 7,
      '4': 1,
      '5': 9,
      '9': 1,
      '10': 'originalBaselineRunId',
      '17': true
    },
    {'1': 'improvements', '3': 8, '4': 1, '5': 13, '10': 'improvements'},
    {'1': 'regressions', '3': 9, '4': 1, '5': 13, '10': 'regressions'},
    {'1': 'unchanged_wrong', '3': 10, '4': 1, '5': 13, '10': 'unchangedWrong'},
    {
      '1': 'unchanged_correct',
      '3': 11,
      '4': 1,
      '5': 13,
      '10': 'unchangedCorrect'
    },
    {'1': 'question_count', '3': 12, '4': 1, '5': 13, '10': 'questionCount'},
    {
      '1': 'sample_classification',
      '3': 13,
      '4': 1,
      '5': 9,
      '10': 'sampleClassification'
    },
    {'1': 'focused', '3': 14, '4': 1, '5': 8, '10': 'focused'},
    {
      '1': 'aggregate_accuracy',
      '3': 15,
      '4': 1,
      '5': 1,
      '9': 2,
      '10': 'aggregateAccuracy',
      '17': true
    },
    {
      '1': 'aggregate_correct',
      '3': 16,
      '4': 1,
      '5': 13,
      '9': 3,
      '10': 'aggregateCorrect',
      '17': true
    },
    {
      '1': 'aggregate_total',
      '3': 17,
      '4': 1,
      '5': 13,
      '9': 4,
      '10': 'aggregateTotal',
      '17': true
    },
  ],
  '8': [
    {'1': '_compared_to_run_id'},
    {'1': '_original_baseline_run_id'},
    {'1': '_aggregate_accuracy'},
    {'1': '_aggregate_correct'},
    {'1': '_aggregate_total'},
  ],
};

/// Descriptor for `TrialMarker`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List trialMarkerDescriptor = $convert.base64Decode(
    'CgtUcmlhbE1hcmtlchIZCghzdGFja19pZBgBIAEoCVIHc3RhY2tJZBIbCgljaGFuZ2VfaWQYAi'
    'ABKAlSCGNoYW5nZUlkEiEKDGNoYW5nZV90aXRsZRgDIAEoCVILY2hhbmdlVGl0bGUSGgoIZGVj'
    'aXNpb24YBCABKAlSCGRlY2lzaW9uEiMKDWFuYWx5c2lzX3BhdGgYBSABKAlSDGFuYWx5c2lzUG'
    'F0aBIwChJjb21wYXJlZF90b19ydW5faWQYBiABKAlIAFIPY29tcGFyZWRUb1J1bklkiAEBEjwK'
    'GG9yaWdpbmFsX2Jhc2VsaW5lX3J1bl9pZBgHIAEoCUgBUhVvcmlnaW5hbEJhc2VsaW5lUnVuSW'
    'SIAQESIgoMaW1wcm92ZW1lbnRzGAggASgNUgxpbXByb3ZlbWVudHMSIAoLcmVncmVzc2lvbnMY'
    'CSABKA1SC3JlZ3Jlc3Npb25zEicKD3VuY2hhbmdlZF93cm9uZxgKIAEoDVIOdW5jaGFuZ2VkV3'
    'JvbmcSKwoRdW5jaGFuZ2VkX2NvcnJlY3QYCyABKA1SEHVuY2hhbmdlZENvcnJlY3QSJQoOcXVl'
    'c3Rpb25fY291bnQYDCABKA1SDXF1ZXN0aW9uQ291bnQSMwoVc2FtcGxlX2NsYXNzaWZpY2F0aW'
    '9uGA0gASgJUhRzYW1wbGVDbGFzc2lmaWNhdGlvbhIYCgdmb2N1c2VkGA4gASgIUgdmb2N1c2Vk'
    'EjIKEmFnZ3JlZ2F0ZV9hY2N1cmFjeRgPIAEoAUgCUhFhZ2dyZWdhdGVBY2N1cmFjeYgBARIwCh'
    'FhZ2dyZWdhdGVfY29ycmVjdBgQIAEoDUgDUhBhZ2dyZWdhdGVDb3JyZWN0iAEBEiwKD2FnZ3Jl'
    'Z2F0ZV90b3RhbBgRIAEoDUgEUg5hZ2dyZWdhdGVUb3RhbIgBAUIVChNfY29tcGFyZWRfdG9fcn'
    'VuX2lkQhsKGV9vcmlnaW5hbF9iYXNlbGluZV9ydW5faWRCFQoTX2FnZ3JlZ2F0ZV9hY2N1cmFj'
    'eUIUChJfYWdncmVnYXRlX2NvcnJlY3RCEgoQX2FnZ3JlZ2F0ZV90b3RhbA==');

@$core.Deprecated('Use runSummaryDescriptor instead')
const RunSummary$json = {
  '1': 'RunSummary',
  '2': [
    {'1': 'run_id', '3': 1, '4': 1, '5': 9, '10': 'runId'},
    {'1': 'origin', '3': 2, '4': 1, '5': 9, '10': 'origin'},
    {'1': 'system', '3': 3, '4': 1, '5': 9, '10': 'system'},
    {'1': 'benchmark', '3': 4, '4': 1, '5': 9, '10': 'benchmark'},
    {'1': 'limit', '3': 5, '4': 1, '5': 13, '9': 0, '10': 'limit', '17': true},
    {'1': 'run_name', '3': 6, '4': 1, '5': 9, '10': 'runName'},
    {'1': 'display_name', '3': 7, '4': 1, '5': 9, '10': 'displayName'},
    {'1': 'run_kind', '3': 8, '4': 1, '5': 9, '10': 'runKind'},
    {'1': 'registry_section', '3': 9, '4': 1, '5': 9, '10': 'registrySection'},
    {'1': 'is_meta_record', '3': 10, '4': 1, '5': 8, '10': 'isMetaRecord'},
    {
      '1': 'tuning_cohort',
      '3': 11,
      '4': 1,
      '5': 9,
      '9': 1,
      '10': 'tuningCohort',
      '17': true
    },
    {
      '1': 'tuning_shape',
      '3': 12,
      '4': 1,
      '5': 9,
      '9': 2,
      '10': 'tuningShape',
      '17': true
    },
    {'1': 'config_label', '3': 13, '4': 1, '5': 9, '10': 'configLabel'},
    {'1': 'settings_label', '3': 14, '4': 1, '5': 9, '10': 'settingsLabel'},
    {
      '1': 'accuracy',
      '3': 15,
      '4': 1,
      '5': 1,
      '9': 3,
      '10': 'accuracy',
      '17': true
    },
    {
      '1': 'accuracy_correct',
      '3': 16,
      '4': 1,
      '5': 13,
      '9': 4,
      '10': 'accuracyCorrect',
      '17': true
    },
    {
      '1': 'accuracy_total',
      '3': 17,
      '4': 1,
      '5': 13,
      '9': 5,
      '10': 'accuracyTotal',
      '17': true
    },
    {
      '1': 'task_averaged_accuracy',
      '3': 18,
      '4': 1,
      '5': 1,
      '9': 6,
      '10': 'taskAveragedAccuracy',
      '17': true
    },
    {
      '1': 'abstention_accuracy',
      '3': 19,
      '4': 1,
      '5': 1,
      '9': 7,
      '10': 'abstentionAccuracy',
      '17': true
    },
    {
      '1': 'cost_micro_usd',
      '3': 20,
      '4': 1,
      '5': 3,
      '9': 8,
      '10': 'costMicroUsd',
      '17': true
    },
    {
      '1': 'latency_ms_p50',
      '3': 21,
      '4': 1,
      '5': 1,
      '9': 9,
      '10': 'latencyMsP50',
      '17': true
    },
    {
      '1': 'latency_ms_p95',
      '3': 22,
      '4': 1,
      '5': 1,
      '9': 10,
      '10': 'latencyMsP95',
      '17': true
    },
    {
      '1': 'config_signature',
      '3': 23,
      '4': 1,
      '5': 9,
      '9': 11,
      '10': 'configSignature',
      '17': true
    },
    {'1': 'cohort_id', '3': 24, '4': 1, '5': 9, '10': 'cohortId'},
    {
      '1': 'dataset_fingerprint',
      '3': 25,
      '4': 1,
      '5': 9,
      '9': 12,
      '10': 'datasetFingerprint',
      '17': true
    },
    {
      '1': 'judge_model',
      '3': 26,
      '4': 1,
      '5': 9,
      '9': 13,
      '10': 'judgeModel',
      '17': true
    },
    {
      '1': 'judge_prompt_mode',
      '3': 27,
      '4': 1,
      '5': 9,
      '9': 14,
      '10': 'judgePromptMode',
      '17': true
    },
    {'1': 'oracle_gold', '3': 28, '4': 1, '5': 8, '10': 'oracleGold'},
    {
      '1': 'created_at',
      '3': 29,
      '4': 1,
      '5': 9,
      '9': 15,
      '10': 'createdAt',
      '17': true
    },
    {
      '1': 'modified_ms',
      '3': 30,
      '4': 1,
      '5': 1,
      '9': 16,
      '10': 'modifiedMs',
      '17': true
    },
    {
      '1': 'per_question_type',
      '3': 31,
      '4': 3,
      '5': 11,
      '6': '.membench.dashboard.v1.RunSummary.PerQuestionTypeEntry',
      '10': 'perQuestionType'
    },
    {
      '1': 'artifacts_available',
      '3': 32,
      '4': 3,
      '5': 9,
      '10': 'artifactsAvailable'
    },
    {
      '1': 'artifacts_missing',
      '3': 33,
      '4': 3,
      '5': 9,
      '10': 'artifactsMissing'
    },
    {
      '1': 'native_state_available',
      '3': 34,
      '4': 1,
      '5': 8,
      '9': 17,
      '10': 'nativeStateAvailable',
      '17': true
    },
    {'1': 'is_trial_run', '3': 35, '4': 1, '5': 8, '10': 'isTrialRun'},
    {
      '1': 'trial_markers',
      '3': 36,
      '4': 3,
      '5': 11,
      '6': '.membench.dashboard.v1.TrialMarker',
      '10': 'trialMarkers'
    },
  ],
  '3': [RunSummary_PerQuestionTypeEntry$json],
  '8': [
    {'1': '_limit'},
    {'1': '_tuning_cohort'},
    {'1': '_tuning_shape'},
    {'1': '_accuracy'},
    {'1': '_accuracy_correct'},
    {'1': '_accuracy_total'},
    {'1': '_task_averaged_accuracy'},
    {'1': '_abstention_accuracy'},
    {'1': '_cost_micro_usd'},
    {'1': '_latency_ms_p50'},
    {'1': '_latency_ms_p95'},
    {'1': '_config_signature'},
    {'1': '_dataset_fingerprint'},
    {'1': '_judge_model'},
    {'1': '_judge_prompt_mode'},
    {'1': '_created_at'},
    {'1': '_modified_ms'},
    {'1': '_native_state_available'},
  ],
};

@$core.Deprecated('Use runSummaryDescriptor instead')
const RunSummary_PerQuestionTypeEntry$json = {
  '1': 'PerQuestionTypeEntry',
  '2': [
    {'1': 'key', '3': 1, '4': 1, '5': 9, '10': 'key'},
    {
      '1': 'value',
      '3': 2,
      '4': 1,
      '5': 11,
      '6': '.membench.dashboard.v1.QTypeScore',
      '10': 'value'
    },
  ],
  '7': {'7': true},
};

/// Descriptor for `RunSummary`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List runSummaryDescriptor = $convert.base64Decode(
    'CgpSdW5TdW1tYXJ5EhUKBnJ1bl9pZBgBIAEoCVIFcnVuSWQSFgoGb3JpZ2luGAIgASgJUgZvcm'
    'lnaW4SFgoGc3lzdGVtGAMgASgJUgZzeXN0ZW0SHAoJYmVuY2htYXJrGAQgASgJUgliZW5jaG1h'
    'cmsSGQoFbGltaXQYBSABKA1IAFIFbGltaXSIAQESGQoIcnVuX25hbWUYBiABKAlSB3J1bk5hbW'
    'USIQoMZGlzcGxheV9uYW1lGAcgASgJUgtkaXNwbGF5TmFtZRIZCghydW5fa2luZBgIIAEoCVIH'
    'cnVuS2luZBIpChByZWdpc3RyeV9zZWN0aW9uGAkgASgJUg9yZWdpc3RyeVNlY3Rpb24SJAoOaX'
    'NfbWV0YV9yZWNvcmQYCiABKAhSDGlzTWV0YVJlY29yZBIoCg10dW5pbmdfY29ob3J0GAsgASgJ'
    'SAFSDHR1bmluZ0NvaG9ydIgBARImCgx0dW5pbmdfc2hhcGUYDCABKAlIAlILdHVuaW5nU2hhcG'
    'WIAQESIQoMY29uZmlnX2xhYmVsGA0gASgJUgtjb25maWdMYWJlbBIlCg5zZXR0aW5nc19sYWJl'
    'bBgOIAEoCVINc2V0dGluZ3NMYWJlbBIfCghhY2N1cmFjeRgPIAEoAUgDUghhY2N1cmFjeYgBAR'
    'IuChBhY2N1cmFjeV9jb3JyZWN0GBAgASgNSARSD2FjY3VyYWN5Q29ycmVjdIgBARIqCg5hY2N1'
    'cmFjeV90b3RhbBgRIAEoDUgFUg1hY2N1cmFjeVRvdGFsiAEBEjkKFnRhc2tfYXZlcmFnZWRfYW'
    'NjdXJhY3kYEiABKAFIBlIUdGFza0F2ZXJhZ2VkQWNjdXJhY3mIAQESNAoTYWJzdGVudGlvbl9h'
    'Y2N1cmFjeRgTIAEoAUgHUhJhYnN0ZW50aW9uQWNjdXJhY3mIAQESKQoOY29zdF9taWNyb191c2'
    'QYFCABKANICFIMY29zdE1pY3JvVXNkiAEBEikKDmxhdGVuY3lfbXNfcDUwGBUgASgBSAlSDGxh'
    'dGVuY3lNc1A1MIgBARIpCg5sYXRlbmN5X21zX3A5NRgWIAEoAUgKUgxsYXRlbmN5TXNQOTWIAQ'
    'ESLgoQY29uZmlnX3NpZ25hdHVyZRgXIAEoCUgLUg9jb25maWdTaWduYXR1cmWIAQESGwoJY29o'
    'b3J0X2lkGBggASgJUghjb2hvcnRJZBI0ChNkYXRhc2V0X2ZpbmdlcnByaW50GBkgASgJSAxSEm'
    'RhdGFzZXRGaW5nZXJwcmludIgBARIkCgtqdWRnZV9tb2RlbBgaIAEoCUgNUgpqdWRnZU1vZGVs'
    'iAEBEi8KEWp1ZGdlX3Byb21wdF9tb2RlGBsgASgJSA5SD2p1ZGdlUHJvbXB0TW9kZYgBARIfCg'
    'tvcmFjbGVfZ29sZBgcIAEoCFIKb3JhY2xlR29sZBIiCgpjcmVhdGVkX2F0GB0gASgJSA9SCWNy'
    'ZWF0ZWRBdIgBARIkCgttb2RpZmllZF9tcxgeIAEoAUgQUgptb2RpZmllZE1ziAEBEmIKEXBlcl'
    '9xdWVzdGlvbl90eXBlGB8gAygLMjYubWVtYmVuY2guZGFzaGJvYXJkLnYxLlJ1blN1bW1hcnku'
    'UGVyUXVlc3Rpb25UeXBlRW50cnlSD3BlclF1ZXN0aW9uVHlwZRIvChNhcnRpZmFjdHNfYXZhaW'
    'xhYmxlGCAgAygJUhJhcnRpZmFjdHNBdmFpbGFibGUSKwoRYXJ0aWZhY3RzX21pc3NpbmcYISAD'
    'KAlSEGFydGlmYWN0c01pc3NpbmcSOQoWbmF0aXZlX3N0YXRlX2F2YWlsYWJsZRgiIAEoCEgRUh'
    'RuYXRpdmVTdGF0ZUF2YWlsYWJsZYgBARIgCgxpc190cmlhbF9ydW4YIyABKAhSCmlzVHJpYWxS'
    'dW4SRwoNdHJpYWxfbWFya2VycxgkIAMoCzIiLm1lbWJlbmNoLmRhc2hib2FyZC52MS5UcmlhbE'
    '1hcmtlclIMdHJpYWxNYXJrZXJzGmUKFFBlclF1ZXN0aW9uVHlwZUVudHJ5EhAKA2tleRgBIAEo'
    'CVIDa2V5EjcKBXZhbHVlGAIgASgLMiEubWVtYmVuY2guZGFzaGJvYXJkLnYxLlFUeXBlU2Nvcm'
    'VSBXZhbHVlOgI4AUIICgZfbGltaXRCEAoOX3R1bmluZ19jb2hvcnRCDwoNX3R1bmluZ19zaGFw'
    'ZUILCglfYWNjdXJhY3lCEwoRX2FjY3VyYWN5X2NvcnJlY3RCEQoPX2FjY3VyYWN5X3RvdGFsQh'
    'kKF190YXNrX2F2ZXJhZ2VkX2FjY3VyYWN5QhYKFF9hYnN0ZW50aW9uX2FjY3VyYWN5QhEKD19j'
    'b3N0X21pY3JvX3VzZEIRCg9fbGF0ZW5jeV9tc19wNTBCEQoPX2xhdGVuY3lfbXNfcDk1QhMKEV'
    '9jb25maWdfc2lnbmF0dXJlQhYKFF9kYXRhc2V0X2ZpbmdlcnByaW50Qg4KDF9qdWRnZV9tb2Rl'
    'bEIUChJfanVkZ2VfcHJvbXB0X21vZGVCDQoLX2NyZWF0ZWRfYXRCDgoMX21vZGlmaWVkX21zQh'
    'kKF19uYXRpdmVfc3RhdGVfYXZhaWxhYmxl');

@$core.Deprecated('Use runsResponseDescriptor instead')
const RunsResponse$json = {
  '1': 'RunsResponse',
  '2': [
    {
      '1': 'runs',
      '3': 1,
      '4': 3,
      '5': 11,
      '6': '.membench.dashboard.v1.RunSummary',
      '10': 'runs'
    },
  ],
};

/// Descriptor for `RunsResponse`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List runsResponseDescriptor = $convert.base64Decode(
    'CgxSdW5zUmVzcG9uc2USNQoEcnVucxgBIAMoCzIhLm1lbWJlbmNoLmRhc2hib2FyZC52MS5SdW'
    '5TdW1tYXJ5UgRydW5z');

@$core.Deprecated('Use pendingRunDescriptor instead')
const PendingRun$json = {
  '1': 'PendingRun',
  '2': [
    {
      '1': 'age_secs',
      '3': 1,
      '4': 1,
      '5': 1,
      '9': 0,
      '10': 'ageSecs',
      '17': true
    },
    {'1': 'benchmark', '3': 2, '4': 1, '5': 9, '10': 'benchmark'},
    {'1': 'config_label', '3': 3, '4': 1, '5': 9, '10': 'configLabel'},
    {'1': 'hypotheses', '3': 4, '4': 1, '5': 13, '10': 'hypotheses'},
    {'1': 'ingested', '3': 5, '4': 1, '5': 13, '10': 'ingested'},
    {'1': 'limit', '3': 6, '4': 1, '5': 13, '9': 1, '10': 'limit', '17': true},
    {'1': 'oracle_gold', '3': 7, '4': 1, '5': 8, '10': 'oracleGold'},
    {'1': 'origin', '3': 8, '4': 1, '5': 9, '10': 'origin'},
    {'1': 'run_id', '3': 9, '4': 1, '5': 9, '10': 'runId'},
    {'1': 'run_name', '3': 10, '4': 1, '5': 9, '10': 'runName'},
    {'1': 'settings_label', '3': 11, '4': 1, '5': 9, '10': 'settingsLabel'},
    {
      '1': 'started_ms',
      '3': 12,
      '4': 1,
      '5': 1,
      '9': 2,
      '10': 'startedMs',
      '17': true
    },
    {'1': 'status', '3': 13, '4': 1, '5': 9, '10': 'status'},
    {'1': 'system', '3': 14, '4': 1, '5': 9, '10': 'system'},
    {
      '1': 'updated_ms',
      '3': 15,
      '4': 1,
      '5': 1,
      '9': 3,
      '10': 'updatedMs',
      '17': true
    },
  ],
  '8': [
    {'1': '_age_secs'},
    {'1': '_limit'},
    {'1': '_started_ms'},
    {'1': '_updated_ms'},
  ],
};

/// Descriptor for `PendingRun`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List pendingRunDescriptor = $convert.base64Decode(
    'CgpQZW5kaW5nUnVuEh4KCGFnZV9zZWNzGAEgASgBSABSB2FnZVNlY3OIAQESHAoJYmVuY2htYX'
    'JrGAIgASgJUgliZW5jaG1hcmsSIQoMY29uZmlnX2xhYmVsGAMgASgJUgtjb25maWdMYWJlbBIe'
    'CgpoeXBvdGhlc2VzGAQgASgNUgpoeXBvdGhlc2VzEhoKCGluZ2VzdGVkGAUgASgNUghpbmdlc3'
    'RlZBIZCgVsaW1pdBgGIAEoDUgBUgVsaW1pdIgBARIfCgtvcmFjbGVfZ29sZBgHIAEoCFIKb3Jh'
    'Y2xlR29sZBIWCgZvcmlnaW4YCCABKAlSBm9yaWdpbhIVCgZydW5faWQYCSABKAlSBXJ1bklkEh'
    'kKCHJ1bl9uYW1lGAogASgJUgdydW5OYW1lEiUKDnNldHRpbmdzX2xhYmVsGAsgASgJUg1zZXR0'
    'aW5nc0xhYmVsEiIKCnN0YXJ0ZWRfbXMYDCABKAFIAlIJc3RhcnRlZE1ziAEBEhYKBnN0YXR1cx'
    'gNIAEoCVIGc3RhdHVzEhYKBnN5c3RlbRgOIAEoCVIGc3lzdGVtEiIKCnVwZGF0ZWRfbXMYDyAB'
    'KAFIA1IJdXBkYXRlZE1ziAEBQgsKCV9hZ2Vfc2Vjc0IICgZfbGltaXRCDQoLX3N0YXJ0ZWRfbX'
    'NCDQoLX3VwZGF0ZWRfbXM=');

@$core.Deprecated('Use pendingResponseDescriptor instead')
const PendingResponse$json = {
  '1': 'PendingResponse',
  '2': [
    {
      '1': 'pending',
      '3': 1,
      '4': 3,
      '5': 11,
      '6': '.membench.dashboard.v1.PendingRun',
      '10': 'pending'
    },
  ],
};

/// Descriptor for `PendingResponse`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List pendingResponseDescriptor = $convert.base64Decode(
    'Cg9QZW5kaW5nUmVzcG9uc2USOwoHcGVuZGluZxgBIAMoCzIhLm1lbWJlbmNoLmRhc2hib2FyZC'
    '52MS5QZW5kaW5nUnVuUgdwZW5kaW5n');

@$core.Deprecated('Use questionRowDescriptor instead')
const QuestionRow$json = {
  '1': 'QuestionRow',
  '2': [
    {'1': 'question_id', '3': 1, '4': 1, '5': 9, '10': 'questionId'},
    {
      '1': 'question_type',
      '3': 2,
      '4': 1,
      '5': 9,
      '9': 0,
      '10': 'questionType',
      '17': true
    },
    {
      '1': 'question',
      '3': 3,
      '4': 1,
      '5': 9,
      '9': 1,
      '10': 'question',
      '17': true
    },
    {
      '1': 'gold_answer',
      '3': 4,
      '4': 1,
      '5': 9,
      '9': 2,
      '10': 'goldAnswer',
      '17': true
    },
    {
      '1': 'hypothesis',
      '3': 5,
      '4': 1,
      '5': 9,
      '9': 3,
      '10': 'hypothesis',
      '17': true
    },
    {'1': 'label', '3': 6, '4': 1, '5': 8, '9': 4, '10': 'label', '17': true},
    {
      '1': 'is_abstention',
      '3': 7,
      '4': 1,
      '5': 8,
      '9': 5,
      '10': 'isAbstention',
      '17': true
    },
    {
      '1': 'judge_raw',
      '3': 8,
      '4': 1,
      '5': 9,
      '9': 6,
      '10': 'judgeRaw',
      '17': true
    },
    {
      '1': 'judge_system_prompt',
      '3': 9,
      '4': 1,
      '5': 9,
      '9': 7,
      '10': 'judgeSystemPrompt',
      '17': true
    },
    {
      '1': 'judge_user_prompt',
      '3': 10,
      '4': 1,
      '5': 9,
      '9': 8,
      '10': 'judgeUserPrompt',
      '17': true
    },
    {
      '1': 'judge_model',
      '3': 11,
      '4': 1,
      '5': 9,
      '9': 9,
      '10': 'judgeModel',
      '17': true
    },
    {
      '1': 'router_pick',
      '3': 12,
      '4': 1,
      '5': 9,
      '9': 10,
      '10': 'routerPick',
      '17': true
    },
    {
      '1': 'initial_pick',
      '3': 13,
      '4': 1,
      '5': 9,
      '9': 11,
      '10': 'initialPick',
      '17': true
    },
    {
      '1': 'final_pick',
      '3': 14,
      '4': 1,
      '5': 9,
      '9': 12,
      '10': 'finalPick',
      '17': true
    },
    {
      '1': 'debug_artifact',
      '3': 15,
      '4': 1,
      '5': 9,
      '9': 13,
      '10': 'debugArtifact',
      '17': true
    },
    {'1': 'error', '3': 16, '4': 1, '5': 9, '9': 14, '10': 'error', '17': true},
  ],
  '8': [
    {'1': '_question_type'},
    {'1': '_question'},
    {'1': '_gold_answer'},
    {'1': '_hypothesis'},
    {'1': '_label'},
    {'1': '_is_abstention'},
    {'1': '_judge_raw'},
    {'1': '_judge_system_prompt'},
    {'1': '_judge_user_prompt'},
    {'1': '_judge_model'},
    {'1': '_router_pick'},
    {'1': '_initial_pick'},
    {'1': '_final_pick'},
    {'1': '_debug_artifact'},
    {'1': '_error'},
  ],
};

/// Descriptor for `QuestionRow`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List questionRowDescriptor = $convert.base64Decode(
    'CgtRdWVzdGlvblJvdxIfCgtxdWVzdGlvbl9pZBgBIAEoCVIKcXVlc3Rpb25JZBIoCg1xdWVzdG'
    'lvbl90eXBlGAIgASgJSABSDHF1ZXN0aW9uVHlwZYgBARIfCghxdWVzdGlvbhgDIAEoCUgBUghx'
    'dWVzdGlvbogBARIkCgtnb2xkX2Fuc3dlchgEIAEoCUgCUgpnb2xkQW5zd2VyiAEBEiMKCmh5cG'
    '90aGVzaXMYBSABKAlIA1IKaHlwb3RoZXNpc4gBARIZCgVsYWJlbBgGIAEoCEgEUgVsYWJlbIgB'
    'ARIoCg1pc19hYnN0ZW50aW9uGAcgASgISAVSDGlzQWJzdGVudGlvbogBARIgCglqdWRnZV9yYX'
    'cYCCABKAlIBlIIanVkZ2VSYXeIAQESMwoTanVkZ2Vfc3lzdGVtX3Byb21wdBgJIAEoCUgHUhFq'
    'dWRnZVN5c3RlbVByb21wdIgBARIvChFqdWRnZV91c2VyX3Byb21wdBgKIAEoCUgIUg9qdWRnZV'
    'VzZXJQcm9tcHSIAQESJAoLanVkZ2VfbW9kZWwYCyABKAlICVIKanVkZ2VNb2RlbIgBARIkCgty'
    'b3V0ZXJfcGljaxgMIAEoCUgKUgpyb3V0ZXJQaWNriAEBEiYKDGluaXRpYWxfcGljaxgNIAEoCU'
    'gLUgtpbml0aWFsUGlja4gBARIiCgpmaW5hbF9waWNrGA4gASgJSAxSCWZpbmFsUGlja4gBARIq'
    'Cg5kZWJ1Z19hcnRpZmFjdBgPIAEoCUgNUg1kZWJ1Z0FydGlmYWN0iAEBEhkKBWVycm9yGBAgAS'
    'gJSA5SBWVycm9yiAEBQhAKDl9xdWVzdGlvbl90eXBlQgsKCV9xdWVzdGlvbkIOCgxfZ29sZF9h'
    'bnN3ZXJCDQoLX2h5cG90aGVzaXNCCAoGX2xhYmVsQhAKDl9pc19hYnN0ZW50aW9uQgwKCl9qdW'
    'RnZV9yYXdCFgoUX2p1ZGdlX3N5c3RlbV9wcm9tcHRCFAoSX2p1ZGdlX3VzZXJfcHJvbXB0Qg4K'
    'DF9qdWRnZV9tb2RlbEIOCgxfcm91dGVyX3BpY2tCDwoNX2luaXRpYWxfcGlja0INCgtfZmluYW'
    'xfcGlja0IRCg9fZGVidWdfYXJ0aWZhY3RCCAoGX2Vycm9y');

@$core.Deprecated('Use questionsResponseDescriptor instead')
const QuestionsResponse$json = {
  '1': 'QuestionsResponse',
  '2': [
    {'1': 'total', '3': 1, '4': 1, '5': 13, '10': 'total'},
    {
      '1': 'questions',
      '3': 2,
      '4': 3,
      '5': 11,
      '6': '.membench.dashboard.v1.QuestionRow',
      '10': 'questions'
    },
  ],
};

/// Descriptor for `QuestionsResponse`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List questionsResponseDescriptor = $convert.base64Decode(
    'ChFRdWVzdGlvbnNSZXNwb25zZRIUCgV0b3RhbBgBIAEoDVIFdG90YWwSQAoJcXVlc3Rpb25zGA'
    'IgAygLMiIubWVtYmVuY2guZGFzaGJvYXJkLnYxLlF1ZXN0aW9uUm93UglxdWVzdGlvbnM=');
