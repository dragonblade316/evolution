use dioxus::prelude::*;

const MAP: Asset = asset!("/assets/map.png");
const SOLVER: Asset = asset!("/assets/solver.webp");

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut enabled = use_signal(|| false);

    rsx! {
        style { {STYLE} }
        main { class: "app",
            header { h1 { "EvoDS" } }
            div { class: "content",
                div { class: "dashboard",
                    section { class: "left-panel",
                        img { class: "solver", src: SOLVER, alt: "Robot solver" }
                        Logs {}
                    }
                    Field {}
                    section { class: "right-panel" }
                }
                footer {
                    EnableStatus {
                        enabled: enabled(),
                        on_enable: move |_| enabled.set(true),
                        on_disable: move |_| enabled.set(false),
                    }
                    CommsStatus {}
                    BatteryStatus {}
                    ShooterStatus {}
                }
            }
        }
    }
}

#[component]
fn Logs() -> Element {
    rsx! {
        div { class: "logs",
            for index in 0..10 {
                LogEntry { key: "log-{index}", index }
            }
        }
    }
}

#[component]
fn LogEntry(index: usize) -> Element {
    rsx! {
        div { class: "log-entry", "Log message {index + 1}" }
    }
}

#[component]
fn Field() -> Element {
    rsx! {
        section { class: "field",
            img { src: MAP, alt: "Field map" }
            div { class: "robot", "aria-label": "Robot position",
                div { class: "robot-direction" }
            }
        }
    }
}

#[component]
fn EnableStatus(
    enabled: bool,
    on_enable: EventHandler<MouseEvent>,
    on_disable: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        section { class: "status enable-status",
            strong { "Robot State:" }
            span { if enabled { "Enabled" } else { "Disabled" } }
            div { class: "buttons",
                button { class: "enable", onclick: move |event| on_enable.call(event), "enable" }
                button { class: "disable", onclick: move |event| on_disable.call(event), "disable" }
            }
        }
    }
}

#[component]
fn CommsStatus() -> Element {
    rsx! {
        section { class: "status",
            StatusRow { label: "Robot Comms:" }
            StatusRow { label: "Controller:" }
        }
    }
}

#[component]
fn BatteryStatus() -> Element {
    rsx! {
        section { class: "status battery",
            span { "Battery %: 40%" }
            span { "Voltage: 12V" }
            span { "Current draw: 5A" }
        }
    }
}

#[component]
fn ShooterStatus() -> Element {
    rsx! {
        section { class: "status",
            StatusRow { label: "Turret status:" }
            for slot in 1..=3 {
                StatusRow { key: "slot-{slot}", label: "Indexer Slot {slot}:" }
            }
        }
    }
}

#[component]
fn StatusRow(label: String) -> Element {
    rsx! {
        div { class: "status-row",
            span { "{label}" }
            i {}
        }
    }
}

// const STYLE: &str = r#"
// * { box-sizing: border-box; }
// html, body, #main { margin: 0; width: 100%; height: 100%; }
// body { background: #121212; color: #f5f5f5; font-family: sans-serif; }
// .app { width: 100%; height: 100%; display: flex; flex-direction: column; }
// header { flex: none; padding: 14px 20px; background: #302b3f; }
// h1 { margin: 0; font-size: 1.35rem; font-weight: 500; }
// .content { min-height: 0; flex: 1; display: flex; flex-direction: column; gap: 5px; padding: 8px; }
// .dashboard { min-height: 0; flex: 1; display: grid; grid-template-columns: minmax(220px, 1fr) minmax(420px, 700px) minmax(140px, 1fr); gap: 15px; }
// .left-panel, .right-panel { min-width: 0; min-height: 0; display: flex; flex-direction: column; gap: 10px; }
// .solver { width: 100%; max-width: 400px; max-height: 42%; object-fit: contain; align-self: center; }
// .logs { min-height: 0; flex: 1; overflow-y: auto; }
// .log-entry { margin: 3px; padding: 5px; border: 6px solid #f44336; border-radius: 4px; }
// .field { position: relative; align-self: center; width: min(100%, 700px); aspect-ratio: 1; overflow: hidden; border-radius: 15px; background: #7c4dff; }
// .field > img { display: block; width: 100%; height: 100%; object-fit: contain; }
// .robot { position: absolute; left: 50%; top: 25%; width: 50px; height: 50px; border: 4px solid #ff5252; transform: translate(-50%, -50%) rotate(-79deg); }
// .robot-direction { position: absolute; left: 100%; top: 50%; width: 0; height: 0; border-top: 12px solid transparent; border-bottom: 12px solid transparent; border-left: 18px solid white; transform: translateY(-50%); }
// footer { flex: none; display: flex; align-items: center; justify-content: center; flex-wrap: wrap; gap: 30px; padding: 8px; border-radius: 15px; background: #512da8; }
// .status { display: flex; flex-direction: column; gap: 2px; white-space: nowrap; }
// .enable-status { align-items: center; }
// .buttons { display: flex; gap: 10px; }
// button { border: 0; padding: 8px 14px; color: white; cursor: pointer; font: inherit; }
// button.enable { background: #2e7d32; }
// button.disable { background: #d32f2f; }
// .status-row { display: flex; align-items: center; gap: 4px; }
// .status-row i { display: block; width: 30px; height: 10px; background: #f44336; }
// .battery { align-items: flex-start; }
// @media (max-width: 900px) {
//     .dashboard { grid-template-columns: minmax(180px, 1fr) minmax(300px, 1fr); overflow-y: auto; }
//     .right-panel { display: none; }
//     .field { width: 100%; }
// }
// "#;
