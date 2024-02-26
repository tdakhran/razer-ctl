use crate::packet::Packet;

use anyhow::{anyhow, Context, Result};
use std::{thread, time};

pub struct Device {
    device: hidapi::HidDevice,
}

impl Device {
    const RAZER_VID: u16 = 0x1532;

    pub fn new(pid: u16) -> Result<Device> {
        let api = hidapi::HidApi::new().context("Failed to create hid api")?;
        let device = api.open(Device::RAZER_VID, pid)?;
        Ok(Device { device })
    }

    pub fn send(&self, report: Packet) -> Result<Packet> {
        // extra byte for report id
        let mut response_buf: Vec<u8> = vec![0x00; 1 + std::mem::size_of::<Packet>()];
        //println!("Report {:?}", report);

        thread::sleep(time::Duration::from_millis(2));
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

        thread::sleep(time::Duration::from_millis(2));
        if response_buf.len() != self.device.get_feature_report(&mut response_buf)? {
            return Err(anyhow!("Response size != {}", response_buf.len()));
        }

        // skip report id byte
        let response = <&[u8] as TryInto<Packet>>::try_into(&response_buf[1..])?;
        //println!("Response {:?}", response);
        response.ensure_matches_report(&report)
    }

    pub fn enumerate() -> Result<()> {
        let api = hidapi::HidApi::new().context("Failed to create hid api")?;
        api.device_list()
            .filter(|info| info.vendor_id() == Device::RAZER_VID)
            .for_each(|info| {
                println!(
                    "RazerDevice {{ vid: 0x{:04x}, pid: 0x{:04x}, manufacturer: {}, product: {} }}",
                    info.vendor_id(),
                    info.product_id(),
                    info.manufacturer_string().unwrap_or_default(),
                    info.product_string().unwrap_or_default(),
                )
            });
        Ok(())
    }
}
