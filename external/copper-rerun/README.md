# Copper JSON Rerun viewer

This app turns the JSON stream produced by Copper's generated log reader into a
Rerun recording. It deliberately depends only on the standard JSON export, so it
does not link against an application's generated CopperList types.

## Environment

Run all Python commands in the repository's Nix environment:

```bash
nix develop
```

The main flake shell supplies Python and `rerun-sdk`; no `pip install` is needed.

## Export and view

The Copper exporter writes whitespace-separated JSON objects rather than one
large array. Redirect its stdout to a file:

```bash
cargo run -q -p masterpiece \
  --features logreader \
  --bin masterpiece-logreader -- \
  logs/cu-test.copper extract-copperlists \
  > /tmp/masterpiece-copperlists.json
```

Open that export using the Masterpiece spatial mappings:

```bash
python external/copper-rerun/copper_rerun.py \
  /tmp/masterpiece-copperlists.json \
  --mapping external/copper-rerun/masterpiece.toml
```

Or save an RRD without opening a viewer:

```bash
python external/copper-rerun/copper_rerun.py \
  /tmp/masterpiece-copperlists.json \
  --mapping external/copper-rerun/masterpiece.toml \
  --save /tmp/masterpiece.rrd
```

For a quick validation, add `--limit 100`. Null payloads are skipped so an empty
Copper message slot does not clear the last valid Rerun value.

## Mappings

Unmapped outputs are named `slot_N` or `slot_N_M`. Their nested numeric fields
are logged as scalar entities and strings as text logs. A TOML mapping can rename
an output and attach spatial semantics to a serialized 4x4 transform:

```toml
[viewer]
time_series = [
  "copper/outputs/drive_command/payload/left",
  "copper/outputs/drive_command/payload/right",
]

[outputs."13.0"]
name = "robot_pose"
kind = "pose"
dimensions = [0.5, 0.5, 0.2]

[outputs."8"]
name = "alliance_transform"
kind = "point"
```

Supported mapping kinds are `generic`, `pose`, and `point`.
Spatial payloads use Copper's column-major `Transform3D` serialization, with
translation stored in the fourth outer array.

The default blueprint contains a 3D view and a time-series view. Only paths in
`viewer.time_series` are plotted initially; all other values remain recorded and
can be added interactively without regenerating the RRD.
