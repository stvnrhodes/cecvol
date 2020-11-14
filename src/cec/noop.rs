use crate::cec::{CECCommand, CECConnection, CECError, LogicalAddress, PhysicalAddress};
use log::info;

pub struct LogOnlyConn;

impl CECConnection for LogOnlyConn {
    fn transmit(&self, cmd: CECCommand) -> Result<(), CECError> {
        info!("faking command {:?}", cmd);
        Ok(())
    }
    fn get_logical_address(&self) -> Result<LogicalAddress, CECError> {
        info!("returning fake logical address");
        Ok(LogicalAddress::Broadcast)
    }
    fn get_physical_address(&self) -> Result<PhysicalAddress, CECError> {
        info!("returning fake physical address");
        Ok(0)
    }
    fn set_tx_callback(&self, _: Box<dyn FnMut(&CECCommand) + Send>) {
        info!("faking tx callback");
    }
    fn set_rx_callback(&self, _: Box<dyn FnMut(&CECCommand) + Send>) {
        info!("faking rx callback");
    }
}
