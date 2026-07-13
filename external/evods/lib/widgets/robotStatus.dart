import 'package:flutter/material.dart';

class Logs extends StatelessWidget {
  Widget build(BuildContext context) {
    return ListView.builder(
      itemCount: 10,
      itemBuilder: (BuildContext ctx, int index) {
        return LogEntry();
      },
    );
  }
}

class LogEntry extends StatelessWidget {
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.all(3.0),
      child: Container(
        decoration: BoxDecoration(
          border: Border.all(color: Colors.red, width: 6),
          borderRadius: BorderRadius.circular(4),
        ),
        child: Text("Log message"),
      ),
    );
  }
}
