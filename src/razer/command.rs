use crate::razer::device::Device;
use crate::razer::packet::Packet;
use crate::razer::types::{
    Cluster, CpuBoost, FanMode, FanZone, GpuBoost, MaxFanSpeedMode, PerfMode,
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

    let r1 = r1?;
    ensure!(r1 == r2?, "Modes do not match");

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
    ensure!(
        [CpuBoost::Boost, CpuBoost::Overclock].contains(&get_cpu_boost(device)?),
        "CPU boost must be one of {:?}",
        [CpuBoost::Boost, CpuBoost::Overclock]
    );
    ensure!(
        get_gpu_boost(device)? == GpuBoost::High,
        "GPU boost must be {:?}",
        GpuBoost::High
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

pub fn print_info(device: &Device) -> Result<()> {
    let (perf_mode, fan_mode) = get_perf_mode(device)?;
    println!("{: <20} {:?}", "Performance mode:", perf_mode);

    if perf_mode == PerfMode::Balanced {
        println!("{: <20} {:?}", "Fan mode:", fan_mode);
        if fan_mode == FanMode::Manual {
            println!(
                "{: <20} {:?}",
                "Fan RPM Zone1:",
                get_fan_rpm(device, FanZone::Zone1)?
            );
            println!(
                "{: <20} {:?}",
                "Fan RPM Zone2:",
                get_fan_rpm(device, FanZone::Zone2)?
            );
        }
    }

    if perf_mode == PerfMode::Custom {
        let cpu_boost = get_cpu_boost(device)?;
        let gpu_boost = get_gpu_boost(device)?;
        println!("{: <20} {:?}", "CPU boost:", cpu_boost);
        println!("{: <20} {:?}", "GPU boost:", gpu_boost);

        if (cpu_boost == CpuBoost::Boost || cpu_boost == CpuBoost::Overclock)
            && (gpu_boost == GpuBoost::High)
        {
            // TODO: getter for max fan speed mode
        }
    }

    Ok(())
}
