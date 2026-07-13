import 'dart:ui' as ui;
import 'dart:async';
import 'package:flutter/services.dart';
import 'package:flutter/material.dart';

class FieldPainter extends CustomPainter {
  // late ui.Image map = loadUiImage("assets/map.png");
  //
  var fieldWidthMeters = 2;
  var fieldHeightMeters = 2;

  var x = 1.0;
  var y = 1.5;
  var theta = 1.38;

  @override
  void paint(Canvas canvas, Size size) {
    final double scaleX = size.width / fieldWidthMeters;
    final double scaleY = size.height / fieldHeightMeters;

    final double robotPixelX = x * scaleX;
    final double robotPixelY = size.height - (y * scaleY);

    canvas.save();
    // Move the canvas origin to the robot's position
    canvas.translate(robotPixelX, robotPixelY);
    // Rotate canvas based on robot heading (negated because Flutter's Y is inverted)
    canvas.rotate(-theta);

    final double robotSize = 50.0; // Size in pixels
    final paint = Paint()
      ..color = Colors.redAccent
      ..strokeWidth = 4
      ..style = PaintingStyle.stroke;

    // Draw robot body (a square/rectangle)
    final rect = Rect.fromCenter(
      center: Offset.zero,
      width: robotSize,
      height: robotSize,
    );
    canvas.drawRect(rect, paint);

    // Draw a direction indicator (front of the robot)
    final frontIndicatorPaint = Paint()
      ..color = Colors.white
      ..style = PaintingStyle.stroke
      ..strokeWidth = 4;

    final path = Path()
      ..moveTo(robotSize / 4, 0)
      ..lineTo(0, -robotSize / 4)
      ..lineTo(0, robotSize / 4)
      ..close();

    canvas.drawPath(path, frontIndicatorPaint);

    canvas.restore();
  }

  @override
  bool shouldRepaint(covariant CustomPainter oldDelegate) => false;
}
