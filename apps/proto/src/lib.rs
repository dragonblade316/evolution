// Bridge channel definitions for the proto runtime.
pub mod bridges {
    use cu29::prelude::*;
    use moteus_bridge::MoteusBridge;

    tx_channels! {
        pub struct MoteusTxChannels : MoteusTxId {
            motor => common::MotorCMD = "moteus/motor/cmd",
        }
    }

    rx_channels! {
        pub struct MoteusRxChannels : MoteusRxId {
            motor => moteus_bridge::messages::MoteusData = "moteus/motor/data",
        }
    }

    pub type ProtoMoteusBridge = MoteusBridge<MoteusTxChannels, MoteusRxChannels>;
}
