import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:flutter_http_debugger/main.dart';

void main() {
  testWidgets('debugger app mounts without Rust/FRB bootstrap', (tester) async {
    await tester.pumpWidget(const DebuggerApp());
    expect(find.byType(MaterialApp), findsOneWidget);
  });
}
