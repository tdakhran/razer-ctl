use crate::packet::Packet;

use anyhow::{anyhow, Context, Result};
use itertools::Itertools;
use std::{thread, time};

pub struct DeviceInfo {
    pub name: &'static str,
    pub pid: u16,
}

pub const SUPPORTED: &[DeviceInfo] = &[
    DeviceInfo {
        name: "Razer Blade 16 2023",
        pid: 0x029f,
    },
    DeviceInfo {
        name: "Razer Blade 14 2023",
        pid: 0x029d,
    },
];

pub struct Device {
    device: hidapi::HidDevice,
    info: DeviceInfo,
}

impl Device {
    const RAZER_VID: u16 = 0x1532;

    pub fn info(&self) -> &DeviceInfo {
        &self.info
    }

    pub fn new(pid: u16, name: &'static str) -> Result<Device> {
        let api = hidapi::HidApi::new().context("Failed to create hid api")?;
        let device = api.open(Device::RAZER_VID, pid)?;
        Ok(Device {
            device,
            info: DeviceInfo { name, pid },
        })
    }

    pub fn send(&self, report: Packet) -> Result<Packet> {
        // extra byte for report id
        let mut response_buf: Vec<u8> = vec![0x00; 1 + std::mem::size_of::<Packet>()];
        //println!("Report {:?}", report);

        thread::sleep(time::Duration::from_micros(1000));
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

        thread::sleep(time::Duration::from_micros(2000));
        if response_buf.len() != self.device.get_feature_report(&mut response_buf)? {
            return Err(anyhow!("Response size != {}", response_buf.len()));
        }

        // skip report id byte
        let response = <&[u8] as TryInto<Packet>>::try_into(&response_buf[1..])?;
        //println!("Response {:?}", response);
        response.ensure_matches_report(&report)
    }

    pub fn enumerate() -> Result<std::vec::Vec<DeviceInfo>> {
        let api = hidapi::HidApi::new().context("Failed to create hid api")?;
        Ok(api
            .device_list()
            .filter(|info| info.vendor_id() == Device::RAZER_VID)
            .map(|info| DeviceInfo {
                name: "",
                pid: info.product_id(),
            })
            .unique_by(|info| info.pid)
            .collect())
    }

    pub fn detect() -> Result<Device> {
        for discovered in Device::enumerate()? {
            for supported in SUPPORTED {
                if supported.pid == discovered.pid {
                    return Device::new(supported.pid, supported.name);
                }
            }
        }
        anyhow::bail!("Device is not supported")
    }
}
