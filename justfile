# Workspace automation helpers.

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
  target/debug/masterpiece-twin 127.0.0.1:9000 "$LIVE_ARCHIVE" &
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
