const cards = new Map();
let socket;
let reconnectTimer;

const motorsElement = document.querySelector("#motors");
const notice = document.querySelector("#notice");
const addCanId = document.querySelector("#add-can-id");
const addMotorButton = document.querySelector("#add-motor");

function send(message) {
  if (socket?.readyState !== WebSocket.OPEN) return false;
  socket.send(JSON.stringify(message));
  return true;
}

function connect() {
  clearTimeout(reconnectTimer);
  const scheme = location.protocol === "https:" ? "wss" : "ws";
  socket = new WebSocket(`${scheme}://${location.host}/ws`);
  socket.addEventListener("open", () => {
    document.body.classList.replace("disconnected", "connected");
    document.querySelector("#connection-state").textContent = "Connected";
    notice.textContent = "";
  });
  socket.addEventListener("message", event => {
    const message = JSON.parse(event.data);
    if (message.type === "snapshot") updateSnapshot(message);
    if (message.type === "export") downloadConfigs(message.files);
    if (message.type === "error") notice.textContent = message.message;
  });
  socket.addEventListener("close", () => {
    document.body.classList.replace("connected", "disconnected");
    document.querySelector("#connection-state").textContent = "Disconnected — retrying…";
    reconnectTimer = setTimeout(connect, 1000);
  });
}

function updateSnapshot(snapshot) {
  document.querySelector("#start").hidden = !snapshot.stopped;
  document.querySelector("#stop").hidden = snapshot.stopped;
  const runState = document.querySelector("#run-state");
  runState.textContent = snapshot.stopped ? "STOPPED" : "RUNNING";
  runState.className = snapshot.stopped ? "warning" : "good";
  const connected = snapshot.motors.filter(motor => motor.connection === "Connected").length;
  document.querySelector("#motor-count").textContent =
    `${snapshot.motors.length} motors (${connected} connected)`;

  const activeIds = new Set();
  for (const motor of snapshot.motors) {
    activeIds.add(motor.id);
    let card = cards.get(motor.id);
    if (!card) {
      card = createMotorCard(motor);
      cards.set(motor.id, card);
      motorsElement.append(card.root);
    }
    updateMotorCard(card, motor);
  }
  for (const [id, card] of cards) {
    if (!activeIds.has(id)) {
      card.root.remove();
      cards.delete(id);
    }
  }
  let empty = motorsElement.querySelector(".empty");
  if (snapshot.motors.length === 0 && !empty) {
    empty = document.createElement("p");
    empty.className = "empty";
    empty.textContent = "No motors added. Use “Add Motor” above to connect to a controller.";
    motorsElement.append(empty);
  } else if (snapshot.motors.length > 0) {
    empty?.remove();
  }
}

function createMotorCard(motor) {
  const root = document.createElement("details");
  root.className = "motor-card";
  root.open = true;
  root.innerHTML = `
    <summary data-field="summary"></summary>
    <div class="motor-body">
      <div class="connection-error" data-field="connection-error"></div>
      <div class="motor-columns">
        <div>
          <section class="section">
            <div class="section-title">Command</div>
            <div class="row">
              <button class="mode-button" data-mode="position">Position Mode</button>
              <button class="mode-button" data-mode="velocity">Velocity Mode</button>
            </div>
            <div class="row position-row"><label>Position:</label><input name="position" type="number" step="0.1"><span>rev</span></div>
            <div class="row"><label>Target Vel:</label><input name="velocity" type="number" step="0.5"><span>rev/s</span></div>
            <div class="row"><label><input name="torque-enabled" type="checkbox"> Torque limit:</label><input name="maximum-torque" type="number" min="0" step="0.1"><span>Nm</span></div>
          </section>

          <section class="section">
            <div class="section-title">Motor configuration</div>
            <div class="row"><label><input name="inverted" type="checkbox"> Inverted</label></div>
            <div class="row"><label>Output / rotor:</label><input name="gear-ratio" type="number" min="0" max="1" step="0.001"></div>
            <div class="hint">0.25 = 4:1 reduction</div>
            <div class="row"><label><input name="position-min-enabled" type="checkbox"> Position min:</label><input name="position-min" type="number" step="0.1"><span>rev</span></div>
            <div class="row"><label><input name="position-max-enabled" type="checkbox"> Position max:</label><input name="position-max" type="number" step="0.1"><span>rev</span></div>
            <div class="hint">Unchecked position limits are written as nan</div>
            <button data-action="apply-motor-config">Apply motor configuration and save to flash</button>
            <div class="status" data-field="motor-config-status"></div>
          </section>

          <section class="section">
            <div class="section-title">Safety configuration</div>
            <fieldset name="safety-fields">
              <div class="row"><label>Max phase current:</label><input name="max-current" type="number" min="0" step="0.1"><span>A</span></div>
              <div class="hint warning">Increasing current beyond the hardware rating can cause damage</div>
              <div class="row"><label>Max velocity:</label><input name="max-velocity" type="number" min="0" step="0.5"><span>rev/s</span></div>
              <div class="row"><label><input name="velocity-limit-enabled" type="checkbox"> Default velocity limit:</label><input name="velocity-limit" type="number" min="0" step="0.5"><span>rev/s</span></div>
              <div class="row"><label><input name="accel-limit-enabled" type="checkbox"> Default acceleration limit:</label><input name="accel-limit" type="number" min="0" step="0.5"><span>rev/s²</span></div>
              <div class="row"><label><input name="regen-limit-enabled" type="checkbox"> Limit supply regeneration:</label><input name="regen-limit" type="number" min="0" step="1"><span>W</span></div>
              <div class="hint warning">Use 0 W for non-regenerative supplies; braking can still exceed this limit</div>
              <div class="hint">Unchecked limits are written as nan (disabled)</div>
              <button type="button" data-action="apply-safety">Apply safety configuration and save to flash</button>
            </fieldset>
            <div class="status" data-field="safety-status"></div>
          </section>

          <section class="section">
            <div class="section-title">PID (configured gains)</div>
            <div class="row"><label>kp:</label><input name="kp" type="number" min="0" step="0.01"></div>
            <div class="row"><label>ki:</label><input name="ki" type="number" min="0" step="0.01"></div>
            <div class="row"><label>kd:</label><input name="kd" type="number" min="0" step="0.001"></div>
            <button data-action="apply-pid">Apply PID and save to flash</button>
            <div class="status" data-field="pid-status"></div>
          </section>

          <section class="section">
            <div class="section-title">Last written</div>
            <div data-field="last-written">pos:— vel:— kp:— ki:— kd:—</div>
          </section>
        </div>

        <div>
          <section class="section">
            <div class="section-title">Telemetry</div>
            <div class="stale" data-field="stale"></div>
            <div class="telemetry" data-field="telemetry"></div>
          </section>
        </div>
      </div>

      <div class="motor-actions">
        <button data-action="remove">✖ Remove</button>
        <button data-action="reconnect">🔄 Reconnect</button>
        <span class="divider"></span>
        <label>Change CAN ID: <input name="change-can-id" type="number" min="1" max="127"></label>
        <button data-action="change-can-id">Apply</button>
      </div>
      <div class="hint">⚠ Changes the local connection; use tview to change the physical controller ID</div>
    </div>`;

  const card = { root, motorId: motor.id, mode: motor.control_mode, configRevision: -1 };
  card.q = selector => root.querySelector(selector);

  for (const button of root.querySelectorAll("[data-mode]")) {
    button.addEventListener("click", () => {
      card.mode = button.dataset.mode;
      updateModeButtons(card);
      sendTarget(card);
    });
  }
  for (const input of [card.q('[name="position"]'), card.q('[name="velocity"]'), card.q('[name="maximum-torque"]')]) {
    input.addEventListener("change", () => sendTarget(card));
  }
  card.q('[name="torque-enabled"]').addEventListener("change", event => {
    card.q('[name="maximum-torque"]').disabled = !event.target.checked;
    sendTarget(card);
  });
  bindOptional(card, "position-min");
  bindOptional(card, "position-max");
  bindOptional(card, "velocity-limit");
  bindOptional(card, "accel-limit");
  bindOptional(card, "regen-limit");

  card.q('[data-action="remove"]').addEventListener("click", () =>
    send({ type: "remove_motor", motor_id: card.motorId }));
  card.q('[data-action="reconnect"]').addEventListener("click", () =>
    send({ type: "reconnect", motor_id: card.motorId }));
  card.q('[data-action="change-can-id"]').addEventListener("click", () => {
    const canId = number(card.q('[name="change-can-id"]'));
    if (canId !== null) send({ type: "change_can_id", motor_id: card.motorId, can_id: canId });
  });
  card.q('[data-action="apply-pid"]').addEventListener("click", () => {
    const kp = number(card.q('[name="kp"]'));
    const ki = number(card.q('[name="ki"]'));
    const kd = number(card.q('[name="kd"]'));
    if ([kp, ki, kd].every(value => value !== null)) {
      send({ type: "apply_pid", motor_id: card.motorId, kp, ki, kd });
    }
  });
  card.q('[data-action="apply-motor-config"]').addEventListener("click", () => {
    const gearRatio = number(card.q('[name="gear-ratio"]'));
    if (gearRatio === null) return;
    send({
      type: "apply_motor_configuration",
      motor_id: card.motorId,
      inverted: card.q('[name="inverted"]').checked,
      gear_ratio: gearRatio,
      position_min: optionalNumber(card, "position-min"),
      position_max: optionalNumber(card, "position-max"),
    });
  });
  card.q('[data-action="apply-safety"]').addEventListener("click", () => {
    const maxCurrent = number(card.q('[name="max-current"]'));
    const maxVelocity = number(card.q('[name="max-velocity"]'));
    if (maxCurrent === null || maxVelocity === null) return;
    send({
      type: "apply_safety_configuration",
      motor_id: card.motorId,
      max_current: maxCurrent,
      max_velocity: maxVelocity,
      default_velocity_limit: optionalNumber(card, "velocity-limit"),
      default_accel_limit: optionalNumber(card, "accel-limit"),
      max_regen_power: optionalNumber(card, "regen-limit"),
    });
  });
  return card;
}

function bindOptional(card, name) {
  const checkbox = card.q(`[name="${name}-enabled"]`);
  const input = card.q(`[name="${name}"]`);
  checkbox.addEventListener("change", () => input.disabled = !checkbox.checked);
}

function setOptionalSupport(card, name, supported) {
  const checkbox = card.q(`[name="${name}-enabled"]`);
  const input = card.q(`[name="${name}"]`);
  checkbox.disabled = !supported;
  checkbox.closest(".row").title = supported ? "" : "Not supported by this controller firmware";
  if (!supported) checkbox.checked = false;
  input.disabled = !supported || !checkbox.checked;
}

function updateMotorCard(card, motor) {
  card.motorId = motor.id;
  card.q('[data-field="summary"]').textContent =
    `🔧 ${motor.label}  │  CAN ID ${motor.can_id}  │  ${motor.connection}  │  ${motor.control_mode === "position" ? "Pos" : "Vel"}`;
  card.q('[data-field="connection-error"]').textContent = motor.connection_error ?? "";
  card.q('[data-action="reconnect"]').hidden = motor.connection === "Connected";
  card.q('[name="change-can-id"]').value = motor.can_id;
  card.q('[data-field="pid-status"]').textContent = motor.pid_status ?? "";
  card.q('[data-field="motor-config-status"]').textContent = motor.motor_config_status ?? "";
  card.q('[data-field="safety-status"]').textContent = motor.safety_status ?? "";
  card.q('[name="safety-fields"]').disabled = !motor.safety_configuration_loaded;
  card.q('[data-field="last-written"]').textContent =
    `pos:${fixed(motor.last_written_position, 2)} vel:${fixed(motor.last_written_velocity, 2)} kp:${fixed(motor.last_written_kp, 2)} ki:${fixed(motor.last_written_ki, 2)} kd:${fixed(motor.last_written_kd, 3)}`;

  if (card.configRevision !== motor.config_revision) {
    card.configRevision = motor.config_revision;
    card.mode = motor.control_mode;
    setNumber(card, "position", motor.position);
    setNumber(card, "velocity", motor.velocity);
    setOptional(card, "maximum-torque", motor.maximum_torque, "torque-enabled");
    card.q('[name="inverted"]').checked = motor.config.inverted;
    setNumber(card, "gear-ratio", motor.config.gear_ratio);
    setOptional(card, "position-min", motor.config.position_min);
    setOptional(card, "position-max", motor.config.position_max);
    setNumber(card, "max-current", motor.config.max_current);
    setNumber(card, "max-velocity", motor.config.max_velocity);
    setOptional(card, "velocity-limit", motor.config.default_velocity_limit);
    setOptional(card, "accel-limit", motor.config.default_accel_limit);
    setOptional(card, "regen-limit", motor.config.max_regen_power);
    setNumber(card, "kp", motor.config.kp);
    setNumber(card, "ki", motor.config.ki);
    setNumber(card, "kd", motor.config.kd);
    updateModeButtons(card);
  }

  setOptionalSupport(card, "velocity-limit", motor.supports_velocity_limit);
  setOptionalSupport(card, "accel-limit", motor.supports_accel_limit);
  setOptionalSupport(card, "regen-limit", motor.supports_regen_limit);

  const stale = card.q('[data-field="stale"]');
  stale.textContent = motor.telemetry_age_ms > 500
    ? `⚠ Stale ${(motor.telemetry_age_ms / 1000).toFixed(1)}s`
    : "";
  updateTelemetry(card.q('[data-field="telemetry"]'), motor.telemetry);
}

function updateModeButtons(card) {
  for (const button of card.root.querySelectorAll("[data-mode]")) {
    button.classList.toggle("active", button.dataset.mode === card.mode);
  }
  card.q(".position-row").hidden = card.mode !== "position";
}

function updateTelemetry(element, telemetry) {
  if (!telemetry) {
    element.innerHTML = '<span class="hint">No telemetry</span>';
    return;
  }
  const temperatureClass = telemetry.temperature > 70 ? "bad" : telemetry.temperature > 50 ? "warning" : "good";
  const fault = telemetry.fault === 0
    ? '<span class="value good">None</span>'
    : `<span class="value bad">${telemetry.fault}</span>`;
  element.innerHTML = `
    <span>Pos:</span><span class="value">${fixed(telemetry.position, 3)} rev</span>
    <span>Vel:</span><span class="value">${fixed(telemetry.velocity, 2)} rev/s</span>
    <span>Torque:</span><span class="value">${fixed(telemetry.torque, 2)} Nm</span>
    <span>Current:</span><span class="value">${fixed(telemetry.current, 2)} A</span>
    <span>Voltage:</span><span class="value">${fixed(telemetry.voltage, 1)} V</span>
    <span>Temp:</span><span class="value ${temperatureClass}">${fixed(telemetry.temperature, 1)} °C</span>
    <span>Mode:</span><span class="value">${telemetry.mode}</span>
    <span>Fault:</span>${fault}`;
}

function sendTarget(card) {
  const position = number(card.q('[name="position"]'));
  const velocity = number(card.q('[name="velocity"]'));
  if (position === null || velocity === null) return;
  send({
    type: "set_target",
    motor_id: card.motorId,
    mode: card.mode,
    position,
    velocity,
    maximum_torque: card.q('[name="torque-enabled"]').checked
      ? number(card.q('[name="maximum-torque"]'))
      : null,
  });
}

function setNumber(card, name, value) {
  card.q(`[name="${name}"]`).value = value;
}

function setOptional(card, name, value, checkboxName = `${name}-enabled`) {
  const checkbox = card.q(`[name="${checkboxName}"]`);
  const input = card.q(`[name="${name}"]`);
  checkbox.checked = value !== null;
  input.disabled = value === null;
  if (value !== null) input.value = value;
}

function optionalNumber(card, name) {
  return card.q(`[name="${name}-enabled"]`).checked
    ? number(card.q(`[name="${name}"]`))
    : null;
}

function number(input) {
  if (!input.checkValidity() || input.value.trim() === "") return null;
  const value = Number(input.value);
  return Number.isFinite(value) ? value : null;
}

function fixed(value, digits) {
  return value === null || value === undefined || !Number.isFinite(value)
    ? "—"
    : Number(value).toFixed(digits);
}

function downloadConfigs(files) {
  for (const file of files) {
    const link = document.createElement("a");
    link.href = URL.createObjectURL(new Blob([file.contents], { type: "text/plain" }));
    link.download = file.name;
    link.click();
    setTimeout(() => URL.revokeObjectURL(link.href), 1000);
  }
  notice.textContent = `Exported ${files.length} configuration${files.length === 1 ? "" : "s"}`;
}

document.querySelector("#start").addEventListener("click", () => send({ type: "start_all" }));
document.querySelector("#stop").addEventListener("click", () => send({ type: "stop_all" }));
document.querySelector("#export").addEventListener("click", () => send({ type: "export_configs" }));
addMotorButton.addEventListener("click", () => {
  const canId = number(addCanId);
  if (canId === null || !send({ type: "add_motor", can_id: canId })) return;
  addCanId.value = Math.min(127, canId + 1);
});

connect();
