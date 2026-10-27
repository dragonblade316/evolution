# Robot deployment and live digital twin

This document describes the Raspberry Pi system configuration, `masterpiece` deployment,
driver station connection, live Copper stream, and PWM setup used by the robot.

## Current addresses and ports

| Purpose | Address | Protocol |
| --- | --- | --- |
| Robot SSH and application deployment | `dragonblade316@evolution.local` | SSH |
| Zenoh router used by the driver station | `evolution.local:7447` | TCP |
| Digital twin receiver on the development PC | `192.168.12.189:9000` | UDP |

`evolution.local` is published by Avahi. On the current network it resolves to
`192.168.12.254`, but scripts use the hostname so that they do not depend on that address.

The twin address is intentionally stored in `apps/masterpiece/copperconfig.ron` as
`TWIN_ADDRESS`. Update that constant and redeploy `masterpiece` if the development PC's
address changes. The PC firewall must allow inbound UDP traffic on port 9000.

## Raspberry Pi system configuration

The `robot` NixOS configuration is exported by `flake.nix`:

```text
nixosConfigurations.robot
```

The relevant modules are:

- `nix/robot.nix`: Raspberry Pi hardware, users, packages, SSH, Avahi, and device-tree
  overlays.
- `nix/networking.nix`: Ethernet, configured home Wi-Fi, and fallback access point
  selection.
- `nix/services/evolution.nix`: robot application service and hardware permissions.
- `nix/services/zenoh.nix`: Zenoh router listening on TCP port 7447.

The system includes the `moteus` command-line package from the `robotics-scripts` flake.
The application service runs `/opt/evolution/bin/evolution` as `dragonblade316` and has
access to the `dialout`, `video`, `input`, `gpio`, and `spi` groups.

### Rebuild from the public GitHub repository

Commit and push configuration changes before rebuilding from GitHub. Run the following on
the Raspberry Pi:

```bash
sudo nixos-rebuild switch --refresh --flake github:dragonblade316/evolution#robot
sudo reboot
```

A reboot is required when the device-tree configuration changes. The flake enables the
`nix-command` and `flakes` experimental features on the robot.

To rebuild a specific pushed revision instead of the repository's default branch:

```bash
sudo nixos-rebuild switch --refresh \
  --flake github:dragonblade316/evolution/<commit-or-branch>#robot
sudo reboot
```

## PWM servo on GPIO12

The binary servo uses Raspberry Pi hardware PWM channel 0. It is wired to **BCM GPIO12**,
which is physical header pin 32.

`nix/robot.nix` applies the `pwm0-gpio12` device-tree overlay. This enables the PWM
controller and selects GPIO12's PWM0 alternate function.

Linux exposes the PWM controller through `/sys/class/pwm/pwmchip0`. Those sysfs files are
owned by root by default, even though the application user belongs to `gpio`.
`nix/services/evolution.nix` therefore runs a privileged `ExecStartPre` helper before every
application start. The helper:

1. grants the `gpio` group access to `export` and `unexport`;
2. exports hardware PWM channel 0 if necessary; and
3. grants the `gpio` group access to `period`, `duty_cycle`, `polarity`, and `enable`.

The `+` prefix on `ExecStartPre` makes only that setup helper run with full privileges. The
robot application itself continues to run as the unprivileged `dragonblade316` user.

The `BinaryServo` task defaults to PWM channel 0, 50 Hz, a 1000 microsecond undeployed
pulse, and a 1500 microsecond deployed pulse. These are hardware-channel numbers, not GPIO
numbers. Do not configure the task with `channel = 12`.

After rebuilding and rebooting, verify the setup with:

```bash
ls -ld /sys/class/pwm/pwmchip0/pwm0
ls -l /sys/class/pwm/pwmchip0/pwm0
systemctl status evolution.service
```

## Camera configuration

The attached Arducam OV9281 advertises `MJPG` and `YUYV`; it does not advertise the
`GREY` FourCC. The default mission therefore captures raw 1280x720 `YUYV` frames from
`/dev/video0`.

Using `GREY` with this camera causes application construction to fail with:

```text
V4l device 0 does not provide a format with the FourCC GREY
```

A 1280x720 YUYV frame is 1,843,200 bytes. Copper's live-stream maximum record size is set
to 2 MiB (`TWIN_MAX_RECORD_BYTES = 2097152`) so a complete CopperList can include the raw
frame and the remaining telemetry.

## Deploying `masterpiece`

From the repository root on the development PC:

```bash
just deploy masterpiece
```

This command:

1. enters the `rpi` cross-compilation shell;
2. builds the `masterpiece` binary for `aarch64-unknown-linux-gnu`;
3. copies it atomically to `/opt/evolution/bin/evolution`; and
4. restarts `evolution.service`.

The default SSH destination is `dragonblade316@evolution.local`. Override it when needed:

```bash
ROBOT_TARGET=user@host just deploy masterpiece
```

Check the service after deployment:

```bash
ssh dragonblade316@evolution.local 'systemctl status evolution.service'
ssh dragonblade316@evolution.local 'journalctl -u evolution.service -n 100 --no-pager'
```

Follow logs continuously with:

```bash
ssh dragonblade316@evolution.local 'journalctl -fu evolution.service'
```

## Driver station

The driver station explicitly connects to the robot's Zenoh router at
`tcp/evolution.local:7447` instead of relying on multicast discovery.

Start it from the development PC:

```bash
cargo run -p evods
```

Verify the router on the robot with:

```bash
ssh dragonblade316@evolution.local 'systemctl status zenohd.service'
ssh dragonblade316@evolution.local 'ss -ltn | grep 7447'
```

## Live Copper stream and digital twin

The real robot runtime streams complete CopperLists over nonblocking UDP. The stream
includes the raw camera frame, detections, estimated pose, commands, and motor state.
Transport packets use a 1200-byte MTU and forward-error correction with one continuous
repair symbol for every four source symbols.

The repository patches crates.io `cu29-logstream` with `external/cu29-logstream`. The local
patch adds configurable receiver record limits, allowing the twin to accept the 2 MiB
records required by camera frames.

Start the twin before deploying or restarting the robot:

```bash
just masterpiece-twin
```

The twin listens on all PC interfaces at `0.0.0.0:9000`. Each invocation writes a
timestamped archive under:

```text
apps/masterpiece/logs/masterpiece_live_YYYYMMDD_HHMMSS.copper
```

Recommended startup order:

1. allow inbound UDP port 9000 on the PC firewall;
2. run `just masterpiece-twin`;
3. run `cargo run -p evods`;
4. run `just deploy masterpiece`; and
5. confirm that `evolution.service` remains active and twin data appears.

To confirm the receiver is listening:

```bash
ss -lunp | grep ':9000'
```

If no stream arrives, confirm that `TWIN_ADDRESS` matches the PC's current address, the
firewall permits UDP 9000, and the robot service is active.

## Validation notes

The following checks cover the changed components:

```bash
cargo check -p evods
cargo check -p masterpiece --bin masterpiece
cargo check -p masterpiece --features sim-debug --bin masterpiece-twin
nix eval --raw .#nixosConfigurations.robot.config.system.build.toplevel.drvPath
```

The PWM0/GPIO12 device-tree overlay and generated `evolution.service` unit can be built on
the development PC. A complete x86-to-aarch64 image build currently stops at the
architecture-specific `moteus` source package; rebuilding natively on the Raspberry Pi
avoids that cross-build mismatch.
