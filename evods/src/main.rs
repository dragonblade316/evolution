use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

use gilrs::Button;
use gilrs::Gilrs;
use gilrs::Event;
use tokio::spawn;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::task::spawn_blocking;

use dioxus::prelude::*;

fn joyruntime(tx: watch::Sender<common::GamePadState>) {
    let mut gilrs = Gilrs::new().expect("GILRS failed to start (how?)");

    let mut active_gamepad = None;

    loop {
        while let Some(Event { id, event, time, .. }) = gilrs.next_event() {
            println!("{:?} New event from {}: {:?}", time, id, event);
            active_gamepad = Some(id);
        }

        // You can also use cached gamepad state
        if let Some(gamepad) = active_gamepad.map(|id| gilrs.gamepad(id)) {
            if gamepad.is_pressed(Button::South) {
                println!("Button South is pressed (XBox - A, PS - X)");
            }
        }
    }
}

fn joygrab() -> common::GamePadState {
    todo!()
}

async fn zruntime(ui_rx: UnboundedReceiver<UICOMMAND>) {
    let session = zenoh::open(zenoh::Config::default()).await.expect("zenoh failed to start (How?!?!)");
    let subscriber = session.declare_subscriber("key/expression").await.unwrap();

    loop {
        //Handle ui triggered state changes.

        //Check for messages from robot and update internal state accordingly
        match subscriber.try_recv().unwrap() {
            _ => {}
        }

        //Update gamepad

        //If state changed, publish data to robot

    }
}

enum UICOMMAND {

}

#[tokio::main]
async fn main() {
    let (zen_tx, mut zen_rx) = watch::channel("");
    let (joy_tx, mut joy_rx) = watch::channel("");
    let (ui_tx , mut ui_rx) = mpsc::unbounded_channel::<UICOMMAND>();

    let _zenoh = spawn(zruntime(ui_rx));

    dioxus::launch(App);
}


#[component]
fn App() -> Element {
    rsx! { "HotDog!" }
}
