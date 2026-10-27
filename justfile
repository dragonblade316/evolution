# Workspace automation helpers.

# Cross-compile one binary for the Raspberry Pi. Usage: just rpi-build proto proto
rpi-build package binary:
  #!/usr/bin/env bash
  set -euo pipefail

  nix develop .#rpi --command bash -c '
    set -euo pipefail
    cargo build --release -p "{{package}}" --bin "{{binary}}"
    install -Dm755 \
      "target/aarch64-unknown-linux-gnu/release/{{binary}}" \
      "target/rpi/{{binary}}"
    patchelf --set-interpreter /lib/ld-linux-aarch64.so.1 --remove-rpath \
      "target/rpi/{{binary}}"
  '

# Build and deploy a robot application. Usage: just deploy masterpiece
deploy package binary="":
  #!/usr/bin/env bash
  set -euo pipefail

  package="{{package}}"
  binary="{{binary}}"
  robot="${ROBOT_TARGET:-dragonblade316@evolution.local}"

  [[ -n "$binary" ]] || binary="$package"

  just rpi-build "$package" "$binary"
  nix develop .#rpi --command rsync -ah --progress \
    "target/rpi/$binary" \
    "$robot:/opt/evolution/bin/.evolution.new"
  nix develop .#rpi --command ssh "$robot" \
    'chmod 0755 /opt/evolution/bin/.evolution.new && mv -f /opt/evolution/bin/.evolution.new /opt/evolution/bin/evolution && sudo "$(readlink -f /run/current-system/sw/bin/systemctl)" restart evolution.service'

# Open a Copper app in Evograph. Usage: just evograph masterpiece
evograph app:
  cargo run -p evograph -- "apps/{{app}}"

# Run the Masterpiece simulation with its driver station.
masterpiece-sim:
  #!/usr/bin/env bash
  set -euo pipefail

  cargo run -p evods &
  EVODS_PID=$!

  cleanup() {
    kill "$EVODS_PID" 2>/dev/null || true
    wait "$EVODS_PID" 2>/dev/null || true
  }
  trap cleanup EXIT INT TERM

  cargo run -p masterpiece --features=sim-debug --bin masterpiece-sim

# Receive the robot's live Copper stream and run its digital twin.
masterpiece-twin:
  #!/usr/bin/env bash
  set -euo pipefail

  LIVE_ARCHIVE="apps/masterpiece/logs/masterpiece_live_$(date +%Y%m%d_%H%M%S).copper"
  cargo run -p masterpiece --features=sim-debug --bin masterpiece-twin -- \
    0.0.0.0:9000 "$LIVE_ARCHIVE"

# Run the PC mission against the CAN-connected robot with its live digital twin.
masterpiece-pc:
  #!/usr/bin/env bash
  set -euo pipefail

  cargo build -p evods
  cargo build -p masterpiece --features=pc,sim-debug \
    --bin masterpiece-pc --bin masterpiece-twin

  PIDS=()
  cleanup() {
    for pid in "${PIDS[@]}"; do
      pkill -TERM -P "$pid" 2>/dev/null || true
    done
    for pid in "${PIDS[@]}"; do
      kill "$pid" 2>/dev/null || true
    done
    for pid in "${PIDS[@]}"; do
      wait "$pid" 2>/dev/null || true
    done
  }
  trap cleanup EXIT INT TERM

  zenohd &
  PIDS+=("$!")

  target/debug/evods &
  PIDS+=("$!")

  LIVE_ARCHIVE="apps/masterpiece/logs/masterpiece_pc_live_$(date +%Y%m%d_%H%M%S).copper"
  target/debug/masterpiece-twin 0.0.0.0:9000 "$LIVE_ARCHIVE" &
  PIDS+=("$!")

  target/debug/masterpiece-pc

# Render the execution DAG from the app config.
rcfg:
  #!/usr/bin/env bash
  set -euo pipefail
  APP_DIR="${APP_DIR:-cu_example_app}"
  ../../target/debug/cu29-rendercfg apps/"${APP_DIR}"/copperconfig.ron --open

# Extract the structured log via the log reader.
log:
  #!/usr/bin/env bash
  set -euo pipefail
  APP_DIR="${APP_DIR:-masterpiece}"
  APP_NAME="${APP_NAME:-${APP_DIR}}"
  RUST_BACKTRACE=1 cargo run -p "${APP_NAME}" --features=logreader --bin "${APP_NAME}-logreader" \
    apps/"${APP_DIR}"/logs/"${APP_NAME}".copper extract-text-log target/debug/cu29_log_index

# Extract CopperLists from the log output.
cl:
  #!/usr/bin/env bash
  set -euo pipefail
  APP_DIR="${APP_DIR:-cu_example_app}"
  APP_NAME="${APP_NAME:-${APP_DIR}}"
  RUST_BACKTRACE=1 cargo run -p "${APP_NAME}" --features=logreader --bin "${APP_NAME}-logreader" \
    apps/"${APP_DIR}"/logs/"${APP_NAME}".copper extract-copperlists

resim:
  #!/usr/bin/env bash
  set -euo pipefail
  APP_DIR="${APP_DIR:-cu_example_app}"
  APP_NAME="${APP_NAME:-${APP_DIR}}"
  RUST_BACKTRACE=1 cargo run -p "${APP_NAME}" --features=sim-debug --bin "${APP_NAME}-resim" -- \
    --log-base "apps/${APP_DIR}/logs/${APP_NAME}.copper"

resim-debug debug_base="":
  #!/usr/bin/env bash
  set -euo pipefail
  APP_DIR="${APP_DIR:-cu_example_app}"
  APP_NAME="${APP_NAME:-${APP_DIR}}"
  DEBUG_BASE="{{debug_base}}"
  if [[ -z "$DEBUG_BASE" ]]; then
    DEBUG_BASE="copper/apps/${APP_NAME}/debug/v1"
  fi
  RUST_BACKTRACE=1 cargo run -p "${APP_NAME}" --features=sim-debug --bin "${APP_NAME}-resim" -- \
    --debug-base "$DEBUG_BASE" \
    --log-base "apps/${APP_DIR}/logs/${APP_NAME}.copper" \
    --replay-log-base "apps/${APP_DIR}/logs/${APP_NAME}_resim.copper"
