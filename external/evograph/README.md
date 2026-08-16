# Evograph

Evograph is a native `egui` editor for Copper 1.1 `copperconfig.ron` task graphs.

Run it with a project path:

```sh
cargo run -p evograph -- path/to/copper-app
```

You can also launch it without a path and use **Open folder…**. Right-click the graph canvas to
add an indexed task or bridge, drag between ports to connect them, and use the inspector to edit
node IDs and free-form RON configuration. The `egui-snarl` canvas supports drag-to-pan and
scroll-wheel zoom without relaying out or resizing nodes as the camera scale changes.
Constants and monitors have separate tabs. `Ctrl/Cmd+S` saves.

Each bridge appears as two canvas nodes backed by one bridge configuration: **RX** is the
source-side node for messages received from the bridge, while **TX** is the sink-side node for
messages sent to it. Selecting, editing, or deleting either side operates on that single bridge.

Canvas positions and the current pan/zoom camera are persisted next to the app manifest in
`.evonode`. This sidecar is updated after moving, adding, renaming, or deleting nodes and after
camera changes; it never changes `copperconfig.ron` merely to store visual layout. Version 1
position-only sidecars remain readable and are upgraded when next saved.

## Discovery and safety

- Cargo metadata identifies the application, its direct dependencies, and local workspace
  dependencies. Rust sources are parsed with `syn` to find Copper task, bridge, and monitor trait
  implementations, port types, concrete bridge aliases, channel declarations, and likely config
  keys.
- Type aliases are canonicalized for connection checks. A graph cannot be saved with type
  mismatches, duplicate producers, cycles, invalid RON values, or invalid constant syntax.
  Unconnected inputs are warnings and do not block saving.
- Choosing a component from a workspace-only dependency stages that package for insertion into
  the application manifest as an inherited workspace dependency on save.
- Unsupported top-level RON sections and unknown fields are preserved. Unchanged sections are
  kept byte-for-byte, including comments. Atomic writes and an external-change check make it safe
  to keep Evograph open alongside a text editor.
- Source and manifest changes trigger a debounced reindex. When the graph is clean, its config is
  reloaded against the new type catalog. Dirty nodes retain their current ports so unsaved work is
  not silently rewritten.

Config-key discovery is intentionally advisory: detected keys are optional shortcuts, and users
can always add arbitrary keys. The transform wizard generates a const-valid Copper 1.1
`cu_spatial_payloads::Transform3D<f64>` expression using unit-typed lengths and angles.
