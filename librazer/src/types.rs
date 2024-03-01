use anyhow::{bail, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use strum_macros::{EnumIter, EnumString};

#[derive(Clone, Copy)]
pub enum Cluster {
    Cpu = 0x01,
    Gpu = 0x02,
}

#[derive(Clone, Copy)]
pub enum FanZone {
    Zone1 = 0x01,
    Zone2 = 0x02,
}

#[derive(EnumIter, Clone, Copy, Debug, PartialEq, ValueEnum)]
pub enum PerfMode {
    Balanced = 0,
    Silent = 5,
    Custom = 4,
}

#[derive(EnumIter, Clone, Copy, Debug, ValueEnum, PartialEq, Serialize, Deserialize)]
pub enum MaxFanSpeedMode {
    Enable = 2,
    Disable = 0,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FanMode {
    Auto = 0,
    Manual = 1,
}

#[derive(EnumIter, Clone, Copy, Debug, ValueEnum, PartialEq, Serialize, Deserialize)]
pub enum CpuBoost {
    Low = 0,
    Medium = 1,
    High = 2,
    Boost = 3,
    Overclock = 4,
}

#[derive(EnumIter, Clone, Copy, Debug, ValueEnum, PartialEq, Serialize, Deserialize)]
pub enum GpuBoost {
    Low = 0,
    Medium = 1,
    High = 2,
}

#[derive(
    EnumString, EnumIter, Clone, Copy, Debug, ValueEnum, PartialEq, Serialize, Deserialize,
)]
pub enum LogoMode {
    Off,
    Breathing,
    Static,
}

#[derive(EnumString, ValueEnum, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum LightsAlwaysOn {
    Enable = 0x03,
    Disable = 0x00,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum BatteryCare {
    Disable = 0x50,
    Enable = 0xd0,
}

impl TryFrom<u8> for GpuBoost {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Low),
            1 => Ok(Self::Medium),
            2 => Ok(Self::High),
            _ => bail!("Failed to convert {} to GpuBoost", value),
        }
    }
}

impl TryFrom<u8> for PerfMode {
    type Error = anyhow::Error;

    fn try_from(perf_mode: u8) -> Result<Self, Self::Error> {
        match perf_mode {
            0 => Ok(Self::Balanced),
            5 => Ok(Self::Silent),
            4 => Ok(Self::Custom),
            _ => bail!("Failed to convert {} to PerformanceMode", perf_mode),
        }
    }
}

impl TryFrom<u8> for FanMode {
    type Error = anyhow::Error;

    fn try_from(fan_mode: u8) -> Result<Self, Self::Error> {
        match fan_mode {
            0 => Ok(Self::Auto),
            1 => Ok(Self::Manual),
            _ => bail!("Failed to convert {} to FanMode", fan_mode),
        }
    }
}

impl TryFrom<u8> for CpuBoost {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Low),
            1 => Ok(Self::Medium),
            2 => Ok(Self::High),
            3 => Ok(Self::Boost),
            4 => Ok(Self::Overclock),
            _ => bail!("Failed to convert {} to CpuBoost", value),
        }
    }
}

impl TryFrom<u8> for LightsAlwaysOn {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(LightsAlwaysOn::Disable),
            3 => Ok(LightsAlwaysOn::Enable),
            _ => bail!("Failed to convert {} to LightsAlwaysOn", value),
        }
    }
}

impl TryFrom<u8> for BatteryCare {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x50 => Ok(BatteryCare::Disable),
            0xd0 => Ok(BatteryCare::Enable),
            _ => bail!("Failed to convert {} to BatteryCare", value),
        }
    }
}

impl TryFrom<u8> for MaxFanSpeedMode {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x02 => Ok(MaxFanSpeedMode::Enable),
            0x00 => Ok(MaxFanSpeedMode::Disable),
            _ => bail!("Failed to convert {} to MaxFanSpeedMode", value),
        }
    }
}
