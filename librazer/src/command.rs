use crate::device::Device;
use crate::packet::Packet;
use crate::types::{
    BatteryCare, Cluster, CpuBoost, FanMode, FanZone, GpuBoost, LightsAlwaysOn, LogoMode,
    MaxFanSpeedMode, PerfMode,
};

use anyhow::{bail, ensure, Result};

fn _send_command(device: &Device, command: u16, args: &[u8]) -> Result<Packet> {
    let response = device.send(Packet::new(command, args))?;
    ensure!(response.get_args().starts_with(args));
    Ok(response)
}

fn _set_perf_mode(device: &Device, perf_mode: PerfMode, fan_mode: FanMode) -> Result<()> {
    if (fan_mode == FanMode::Manual) && (perf_mode != PerfMode::Balanced) {
        bail!("{:?} allowed only in {:?}", fan_mode, PerfMode::Balanced);
    }

    [1, 2].into_iter().try_for_each(|zone| {
        _send_command(
            device,
            0x0d02,
            &[0x01, zone, perf_mode as u8, fan_mode as u8],
        )
        .map(|_| ())
    })
}

fn _set_boost(device: &Device, cluster: Cluster, boost: u8) -> Result<()> {
    let args = &[0, cluster as u8, boost];
    ensure!(
        get_perf_mode(device)? == (PerfMode::Custom, FanMode::Auto),
        "Performance mode must be {:?}",
        PerfMode::Custom
    );
    ensure!(device
        .send(Packet::new(0x0d07, args))?
        .get_args()
        .starts_with(args));
    Ok(())
}

fn _get_boost(device: &Device, cluster: Cluster) -> Result<u8> {
    let response = device.send(Packet::new(0x0d87, &[0, cluster as u8, 0]))?;
    ensure!(response.get_args()[1] == cluster as u8);
    Ok(response.get_args()[2])
}

pub fn set_perf_mode(device: &Device, perf_mode: PerfMode) -> Result<()> {
    _set_perf_mode(device, perf_mode, FanMode::Auto)
}

pub fn get_perf_mode(device: &Device) -> Result<(PerfMode, FanMode)> {
    let [r1, r2]: [Result<(PerfMode, FanMode)>; 2] = [1, 2].map(|zone| {
        let response = device.send(Packet::new(0x0d82, &[0, zone, 0, 0]))?;
        Ok((
            PerfMode::try_from(response.get_args()[2])?,
            FanMode::try_from(response.get_args()[3])?,
        ))
    });

    ensure!(
        r1.is_ok() && r2.is_ok(),
        "Failed to get performance mode and fan mode: {:?} {:?}",
        r1,
        r2
    );

    let r1 = r1?;
    let r2 = r2?;

    //let r1 = r1?;
    ensure!(r1 == r2, "Modes do not match: {:?} {:?}", r1, r2);

    Ok(r1)
}

pub fn set_cpu_boost(device: &Device, boost: CpuBoost) -> Result<()> {
    _set_boost(device, Cluster::Cpu, boost as u8)
}

pub fn set_gpu_boost(device: &Device, boost: GpuBoost) -> Result<()> {
    _set_boost(device, Cluster::Gpu, boost as u8)
}

pub fn get_cpu_boost(device: &Device) -> Result<CpuBoost> {
    CpuBoost::try_from(_get_boost(device, Cluster::Cpu)?)
}

pub fn get_gpu_boost(device: &Device) -> Result<GpuBoost> {
    GpuBoost::try_from(_get_boost(device, Cluster::Gpu)?)
}

pub fn set_fan_rpm(device: &Device, rpm: u16) -> Result<()> {
    ensure!((2000..=5000).contains(&rpm));
    ensure!(
        get_perf_mode(device)? == (PerfMode::Balanced, FanMode::Manual),
        "Performance mode must be {:?} and fan mode must be {:?}",
        PerfMode::Balanced,
        FanMode::Manual
    );
    [FanZone::Zone1, FanZone::Zone2]
        .into_iter()
        .try_for_each(|zone| {
            _send_command(device, 0x0d01, &[0, zone as u8, (rpm / 100) as u8]).map(|_| ())
        })
}

pub fn get_fan_rpm(device: &Device, fan_zone: FanZone) -> Result<u16> {
    let response = device.send(Packet::new(0x0d81, &[0, fan_zone as u8, 0]))?;
    ensure!(response.get_args()[1] == fan_zone as u8);
    Ok(response.get_args()[2] as u16 * 100)
}

pub fn set_max_fan_speed_mode(device: &Device, mode: MaxFanSpeedMode) -> Result<()> {
    ensure!(
        get_perf_mode(device)?.0 == PerfMode::Custom,
        "Performance mode must be {:?}",
        PerfMode::Custom
    );
    _send_command(device, 0x070f, &[mode as u8]).map(|_| ())
}

pub fn set_fan_mode(device: &Device, mode: FanMode) -> Result<()> {
    ensure!(
        get_perf_mode(device)?.0 == PerfMode::Balanced,
        "Performance mode must be {:?}",
        PerfMode::Balanced
    );
    _set_perf_mode(device, PerfMode::Balanced, mode)
}

pub fn custom_command(device: &Device, command: u16, args: &[u8]) -> Result<()> {
    let report = Packet::new(command, args);
    println!("Report   {:?}", report);
    let response = device.send(report)?;
    println!("Response {:?}", response);
    Ok(())
}

fn _set_logo_power(device: &Device, mode: LogoMode) -> Result<Packet> {
    match mode {
        LogoMode::Off => _send_command(device, 0x0300, &[1, 4, 0]),
        LogoMode::Static | LogoMode::Breathing => _send_command(device, 0x0300, &[1, 4, 1]),
    }
}

fn _set_logo_mode(device: &Device, mode: LogoMode) -> Result<Packet> {
    match mode {
        LogoMode::Static => _send_command(device, 0x0302, &[1, 4, 0]),
        LogoMode::Breathing => _send_command(device, 0x0302, &[1, 4, 2]),
        _ => bail!("Invalid logo mode"),
    }
}

fn _get_logo_power(device: &Device) -> Result<bool> {
    match device.send(Packet::new(0x0380, &[1, 4, 0]))?.get_args()[2] {
        0 => Ok(false),
        1 => Ok(true),
        _ => bail!("Invalid logo power state"),
    }
}

fn _get_logo_mode(device: &Device) -> Result<LogoMode> {
    match device.send(Packet::new(0x0382, &[1, 4, 0]))?.get_args()[2] {
        0 => Ok(LogoMode::Static),
        2 => Ok(LogoMode::Breathing),
        _ => bail!("Invalid logo power state"),
    }
}

pub fn get_logo_mode(device: &Device) -> Result<LogoMode> {
    let power = _get_logo_power(device)?;
    match power {
        true => _get_logo_mode(device),
        false => Ok(LogoMode::Off),
    }
}

pub fn set_logo_mode(device: &Device, mode: LogoMode) -> Result<()> {
    if mode != LogoMode::Off {
        _set_logo_mode(device, mode)?;
    }
    _set_logo_power(device, mode)?;
    Ok(())
}

pub fn get_keyboard_brightness(device: &Device) -> Result<u8> {
    let response = device.send(Packet::new(0x0383, &[1, 5, 0]))?;
    ensure!(response.get_args()[1] == 5);
    Ok(response.get_args()[2])
}

pub fn set_keyboard_brightness(device: &Device, brightness: u8) -> Result<()> {
    let args = &[1, 5, brightness];
    ensure!(device
        .send(Packet::new(0x0303, args))?
        .get_args()
        .starts_with(args));
    Ok(())
}

pub fn get_lights_always_on(device: &Device) -> Result<LightsAlwaysOn> {
    device.send(Packet::new(0x0084, &[0, 0]))?.get_args()[0].try_into()
}

pub fn set_lights_always_on(device: &Device, lights_always_on: LightsAlwaysOn) -> Result<()> {
    let args = &[lights_always_on as u8, 0];
    ensure!(device
        .send(Packet::new(0x0004, args))?
        .get_args()
        .starts_with(args));
    Ok(())
}

pub fn get_battery_care(device: &Device) -> Result<BatteryCare> {
    device.send(Packet::new(0x0792, &[0]))?.get_args()[0].try_into()
}

pub fn set_battery_care(device: &Device, mode: BatteryCare) -> Result<()> {
    let args = &[mode as u8];
    ensure!(device
        .send(Packet::new(0x0712, args))?
        .get_args()
        .starts_with(args));
    Ok(())
}
