use crate::packet::Packet;

use anyhow::{anyhow, Context, Result};
use std::{thread, time};

pub struct DeviceInfo {
    pub name: &'static str,
    pub pid: u16,
    pub path: Option<String>,
}

pub const SUPPORTED: &[DeviceInfo] = &[
    DeviceInfo {
        name: "Razer Blade 16 2023",
        pid: 0x029f,
        path: None,
    },
    DeviceInfo {
        name: "Razer Blade 14 2023",
        pid: 0x029d,
        path: None,
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

        // there are multiple devices with the same pid, pick first that support feature report
        for info in api
            .device_list()
            .filter(|info| (info.vendor_id(), info.product_id()) == (Device::RAZER_VID, pid))
        {
            let path = info.path();
            let device = api.open_path(path)?;
            if device.send_feature_report(&[0, 0]).is_ok() {
                return Ok(Device {
                    device,
                    info: DeviceInfo {
                        name,
                        pid,
                        path: Some(path.to_str().unwrap().to_string()),
                    },
                });
            }
        }
        anyhow::bail!(
            "No device with pid 0x{:04x} and feature report support found",
            pid
        )
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
                path: Some(info.path().to_str().unwrap().to_string()),
            })
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
