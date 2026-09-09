use cu29::{CopperListTuple, CuPayloadRawBytes, CuResult, UnifiedLogType, prelude::UnifiedLoggerIOReader};
use rerun::RecordingStream;



struct Visualizer {
    stream: RecordingStream
} 

impl Visualizer {

    fn log_copperlist<P>() -> CuResult<()>
        where
        P: CopperListTuple + CuPayloadRawBytes + 'static,
    {
        todo!()
    }
    
}

fn from_log<P>() 
    where
    P: CopperListTuple + CuPayloadRawBytes + 'static,
{
    let mut reader = UnifiedLoggerIOReader::new(dl, UnifiedLogType::CopperList);
}
