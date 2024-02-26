use crate::razer::Packet;
use anyhow::{anyhow, Context, Result};
use std::{thread, time};

pub struct Device {
    device: hidapi::HidDevice,
}

impl Device {
    pub fn new(vid: u16, pid: u16) -> Result<Device> {
        let api = hidapi::HidApi::new().context("Failed to create hid api")?;
        let device = api.open(vid, pid)?;
        Ok(Device { device })
    }

    pub fn send(&self, report: Packet) -> Result<Packet> {
        // extra byte for report id
        let mut response_buf: Vec<u8> = vec![0x00; 1 + std::mem::size_of::<Packet>()];

        thread::sleep(time::Duration::from_millis(1));
        self.device
            .send_feature_report(
                [0_u8; 1] // report id
                    .iter()
                    .copied()
                    .chain(Into::<Vec<u8>>::into(&report).into_iter())
                    .collect::<Vec<_>>()
                    .as_slice(),
            )
            .context("Failed to send feature report")?;

        thread::sleep(time::Duration::from_millis(1));
        if response_buf.len() != self.device.get_feature_report(&mut response_buf)? {
            return Err(anyhow!("Response size != {}", response_buf.len()));
        }

        // skip report id byte
        <&[u8] as TryInto<Packet>>::try_into(&response_buf[1..])?.ensure_matches_report(&report)
    }
}