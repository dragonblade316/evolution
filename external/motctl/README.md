# motctl

`motctl` is a browser-based console for connecting to, exercising, configuring, and
exporting multiple moteus motor controllers. The native Rust process owns the CAN
transport; the browser is only a user interface connected over a WebSocket.

## Safety

Treat the UI as live motor-control equipment.

- Secure the robot before enabling any motor.
- Keep a physical power disconnect within reach.
- Verify CAN IDs, gearing, inversion, position limits, current limits, and velocity limits
  before pressing **Start All**.
- Do not expose port 8080 to an untrusted network. `motctl` does not provide authentication
  or TLS.
- Only one browser may control a running process. If that browser disconnects, `motctl`
  sends stop commands to every connected motor.

The worker starts in the stopped state. Adding or reconnecting a controller does not start
it. **Start All** enables periodic position commands for all connected motors, while
**STOP ALL** immediately returns the worker to its stopped state and sends a moteus stop
command to each controller.

## Running locally

Connect the supported moteus CAN adapter, then run:

```bash
cargo run -p motctl
```

Open:

```text
http://127.0.0.1:8080
```

The server listens on `0.0.0.0:8080` by default. Override the listen address when the UI
should only be accessible locally or must use another port:

```bash
MOTCTL_ADDRESS=127.0.0.1:8080 cargo run -p motctl
```

The moteus crate performs transport discovery. Supported transport depends on the host and
attached hardware, and normally includes fdcanusb and Linux SocketCAN.

## Running on the Raspberry Pi

Cross-build the binary with the workspace helper:

```bash
just rpi-build motctl motctl
```

The resulting executable is:

```text
target/rpi/motctl
```

Copy and run it on the robot, then open `http://evolution.local:8080`. The robot NixOS
configuration allows TCP port 8080 through its firewall. `motctl` is not installed as the
`evolution.service`; run it deliberately while tuning and ensure it is stopped before
returning control to the normal robot application.

## Connecting motors

Enter a moteus CAN ID from 1 through 127 and select **Add Motor**. Duplicate CAN IDs are
rejected. Connection happens on a worker thread so the web server remains responsive.

When a controller connects, `motctl` runs `conf enumerate` and loads the settings it knows
how to edit:

- position PID gains (`kp`, `ki`, and `kd`);
- output inversion and rotor-to-output gear ratio;
- minimum and maximum output position;
- maximum current and velocity;
- default velocity and acceleration limits when supported; and
- maximum regenerative power when supported.

Unavailable settings remain disabled or retain visible defaults. A controller with a
communication error can be reconnected without restarting the server, and its CAN ID can
be changed in the UI.

## Motion commands

Each connected motor can use one of two control modes:

- **Position:** sends the requested output position and velocity.
- **Velocity:** sends `position = NaN`, the moteus sentinel for velocity-only tracking, and
  the requested velocity.

Commands are written at most every 50 ms. The native process clamps requested position to
plus or minus 1000 revolutions and velocity to plus or minus 500 revolutions per second.
An optional non-negative maximum torque can be attached to position commands.

The UI displays returned position, velocity, torque, q-axis current, voltage, temperature,
mode, fault, and telemetry age.

## Persistent controller configuration

Configuration actions stop the affected controller before using its diagnostic stream.
Successful updates issue `conf write`, so they persist in moteus flash.

### PID

Applying PID writes:

```text
servo.pid_position.kp
servo.pid_position.ki
servo.pid_position.kd
servo.pid_position.ilimit = 0.0
```

Gains must be finite and non-negative.

### Motor configuration

Motor configuration writes output sign, rotor-to-output ratio, and optional output
position limits. Gear ratio must be finite and greater than zero. If both position limits
are present, the minimum must be less than the maximum. An omitted limit is written as
`NaN`.

### Safety configuration

Current and velocity limits must first be loaded successfully from the controller before
safety editing is enabled. Maximum current and maximum velocity must be finite and greater
than zero. Optional velocity and acceleration defaults must also be greater than zero;
regenerative power must be non-negative.

Firmware-dependent settings are written only when they were present in the enumerated
controller configuration.

## Exporting configurations

Select **Export configs** to run `conf enumerate` for every connected controller. The
browser downloads one file per controller using this naming scheme:

```text
moteus-ID-<can-id>.cfg
```

Export these files before tuning when a rollback copy is required.

## HTTP and WebSocket interface

The HTML, JavaScript, and CSS are embedded into the executable at compile time. The server
exposes:

| Route | Purpose |
| --- | --- |
| `/` | Control UI |
| `/app.js` | Embedded browser application |
| `/style.css` | Embedded stylesheet |
| `/ws` | JSON WebSocket command and telemetry channel |

Only one WebSocket client is accepted at a time. A second client receives an error and is
closed. Closing the controlling connection triggers **Stop All**.
