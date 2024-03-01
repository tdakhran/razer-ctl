use anyhow::{ensure, Result};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

/// Packet is the structure of the packet that is sent to the Razer HID device and received back.
/// Source https://github.com/Razer-Linux/razer-laptop-control-no-dkms/blob/main/razer_control_gui/src/device.rs.
#[repr(C)]
#[derive(Serialize, Deserialize, Debug)]
pub struct Packet {
    status: u8,
    id: u8,
    remaining_packets: u16,
    protocol_type: u8,
    data_size: u8,
    command_class: u8,
    command_id: u8,
    #[serde(with = "BigArray")]
    args: [u8; 80],
    crc: u8,
    reserved: u8,
}

enum CommandStatus {
    New = 0x00,
    Successful = 0x02,
    NotSupported = 0x05,
}

impl Packet {
    pub fn new(command: u16, args: &[u8]) -> Packet {
        let mut args_buffer = [0x00; 80];
        args_buffer[..args.len()].copy_from_slice(args);

        Packet {
            status: CommandStatus::New as u8,
            id: rand::thread_rng().gen(),
            remaining_packets: 0x0000,
            protocol_type: 0x00,
            data_size: args.len() as u8,
            command_class: (command >> 8) as u8,
            command_id: (command & 0xff) as u8,
            args: args_buffer,
            crc: 0x00,
            reserved: 0x00,
        }
    }

    pub fn set_args(&mut self, args: &[u8]) {
        self.args[..args.len()].copy_from_slice(args)
    }

    pub fn get_args(&self) -> &[u8] {
        &self.args
    }

    pub fn ensure_matches_report(self, report: &Packet) -> Result<Self> {
        ensure!(
            (report.command_class, report.command_id, report.id)
                == (self.command_class, self.command_id, self.id),
            "Response does not match the report"
        );

        ensure!(
            self.remaining_packets == report.remaining_packets
            || (self.command_class, self.command_id) == (0x07, 0x92) /* 0x0792 (bho) has special handling */
            || (self.command_class, self.command_id) == (0x07, 0x8f), /* 0x078f max fan speed mode has special handling */
            "Response command does not match the report"
        );

        ensure!(
            self.status != CommandStatus::NotSupported as u8,
            "Command not supported"
        );

        ensure!(
            self.status == CommandStatus::Successful as u8,
            "Command failed with unknown status: {:02X?}",
            self.status
        );

        Ok(self)
    }
}

impl From<&Packet> for Vec<u8> {
    fn from(packet: &Packet) -> Vec<u8> {
        bincode::serialize(packet).unwrap()
    }
}

impl TryFrom<&[u8]> for Packet {
    type Error = anyhow::Error;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        ensure!(
            data.len() == std::mem::size_of::<Packet>(),
            "Invalid raw data size"
        );

        Ok(bincode::deserialize::<Packet>(data)?)
    }
}
