//This file exists mostly to configure the moteus and zenoh bridges. The reason it is a seperate file is bc these defs will be needed for both the sim.rs and main.rs apps
pub mod bridges {
    use cu29::prelude::*;
    use cu_zenoh_bridge::ZenohBridge;
    use moteus_bridge::MoteusBridge;

    // drive_left => common::MotorCMD = "moteus/drive_left/cmd",

tx_channels! {
        pub struct MoteusTxChannels : MoteusTxId {
            drive_left => common::MotorCMD = "moteus/drive_left/cmd",
            drive_right => common::MotorCMD = "moteus/drive_right/cmd",
            indexer => common::MotorCMD = "moteus/indexer/cmd",
            turret => common::MotorCMD = "moteus/turret/cmd",
            shooter => common::MotorCMD = "moteus/shooter/cmd",
        }
    }

rx_channels! {
        pub struct MoteusRxChannels : MoteusRxId {
            drive_left_data => moteus_bridge::messages::MoteusData = "moteus/drive_left/data",
            drive_right_data => moteus_bridge::messages::MoteusData = "moteus/drive_right/data",
            indexer_data => moteus_bridge::messages::MoteusData = "moteus/indexer/data",
            turret_data => moteus_bridge::messages::MoteusData = "moteus/turret/data",
            shooter_data => moteus_bridge::messages::MoteusData = "moteus/shooter/data",
        }
    }

    pub type UziMoteusBridge = MoteusBridge<MoteusTxChannels, MoteusRxChannels>;

    tx_channels! {
        pub struct EvoDsTxChannels : EvoDsTxId {
            telemetry => cu_spatial_payloads::Pose<f64> = "evods/tel/pose",
        }
    }

    rx_channels! {
        pub struct EvoDsRxChannels : EvoDsRxId {
            command => evods::cu29::payloads::DsTxMsg = "evods/ds",
        }
    }

    pub type EvoDsBridge = ZenohBridge<EvoDsTxChannels, EvoDsRxChannels>;
}

#[cfg(test)]
mod tests {
    use cu29::prelude::TypePath;
    use cu29::units::si::f32::{Angle, Velocity};

    fn assert_type_path<T: TypePath>() {}

    #[test]
    fn copper_unit_payloads_have_type_paths() {
        assert_type_path::<Angle>();
        assert_type_path::<Velocity>();
    }
}
