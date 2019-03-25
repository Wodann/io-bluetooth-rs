#[macro_use]
extern crate cfg_if;

#[cfg(windows)]
extern crate winapi;

pub mod bt;

mod sys;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
pub enum BtProtocol {
    L2CAP,
    RFCOMM,
}

/// A Bluetooth address, consisting of 6 bytes.
#[derive(Clone)]
pub struct BtAddr(pub [u8; 6]);

impl BtAddr {
    pub fn nap_sap(nap: u16, sap: u32) -> Self {
        let nap = nap.to_le_bytes();
        let sap = sap.to_le_bytes();
        Self([sap[0], sap[1], sap[2], sap[3], nap[0], nap[1]])
    }
}

impl std::fmt::Display for BtAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.0[5], self.0[4], self.0[3], self.0[2], self.0[1], self.0[0]
        )
    }
}
