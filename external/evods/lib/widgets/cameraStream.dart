import 'dart:typed_data';

import 'package:flutter/material.dart';

//this widget is mostly just ai bc I did not want to have to figure it out.
class CameraStream extends StatelessWidget {
  final Stream<Uint8List> frameStream;

  const CameraStream({super.key, required this.frameStream});

  Widget build(BuildContext context) {
    return Container(
      child: StreamBuilder<Uint8List>(
        stream: frameStream,
        builder: (context, snapshot) {
          if (!snapshot.hasData) {
            return const Center(
              child: CircularProgressIndicator(color: Colors.deepPurpleAccent),
            );
          }

          return Image.memory(
            snapshot.data!,
            gaplessPlayback:
                true, // CRUCIAL: Prevents a flicker/blank screen between frames
            fit: BoxFit.contain,
            // Optimize image rendering cache size if you know the target dimensions
            // cacheWidth: 640,
          );
        },
      ),
    );
  }
}
