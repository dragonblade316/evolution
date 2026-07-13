import 'package:flutter/material.dart';

class BottomBar extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return FittedBox(
      child: Row(
        mainAxisAlignment: .center,
        mainAxisSize: .max,
        spacing: 30,
        children: [
          EnableStatus(),
          CommsStatus(),
          BatteryStatus(),
          ShooterStatus(),
        ],
      ),
    );
  }
}

class EnableStatus extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Container(
      // decoration: BoxDecoration(
      //   border: Border.all(
      //     color: Colors.purple,
      //     width: 2.0,
      //   ),
      //   borderRadius: BorderRadius.circular(8.0)
      // ),
      child: Column(
        children: [
          Text("Robot State:"),
          Text("Shooting"),
          Row(
            spacing: 10,
            children: [
              TextButton(
                onPressed: () => {},
                child: Text("enable"),
                style: TextButton.styleFrom(
                  backgroundColor: Colors.green,
                  foregroundColor: Colors.white,
                ),
              ),
              TextButton(
                onPressed: () => {},
                child: Text("disable"),
                style: TextButton.styleFrom(
                  backgroundColor: Colors.red,
                  foregroundColor: Colors.white,
                ),
              ),
            ],
            mainAxisAlignment: .center,
          ),
        ],
      ),
    );
  }
}

class CommsStatus extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Container(
      child: Column(
        crossAxisAlignment: .end,
        children: [
          Row(
            children: [
              Text("Robot Comms: "),
              SizedBox(
                width: 30,
                height: 10,
                child: ColoredBox(color: Colors.red),
              ),
            ],
          ),
          Row(
            children: [
              Text("Controller: "),
              SizedBox(
                width: 30,
                height: 10,
                child: ColoredBox(color: Colors.red),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

class BatteryStatus extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Container(
      child: Column(
        crossAxisAlignment: .start,
        children: [
          Text("Battery %: 40%"),
          Text("Voltage: 12V"),
          Text("Current draw: 5A"),
        ],
      ),
    );
  }
}

class ShooterStatus extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Container(
      child: Column(
        crossAxisAlignment: .end,
        children: [
          Row(
            children: [
              Text("Turret status: "),
              SizedBox(
                width: 30,
                height: 10,
                child: ColoredBox(color: Colors.red),
              ),
            ],
          ),
          Row(
            children: [
              Text("Indexer Slot 1: "),
              SizedBox(
                width: 30,
                height: 10,
                child: ColoredBox(color: Colors.red),
              ),
            ],
          ),
          Row(
            children: [
              Text("Indexer Slot 2: "),
              SizedBox(
                width: 30,
                height: 10,
                child: ColoredBox(color: Colors.red),
              ),
            ],
          ),
          Row(
            children: [
              Text("Indexer Slot 3: "),
              SizedBox(
                width: 30,
                height: 10,
                child: ColoredBox(color: Colors.red),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
