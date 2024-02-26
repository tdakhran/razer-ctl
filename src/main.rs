mod razer;

use razer::command;
use razer::device;
use razer::types;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};

pub fn create_device(pid: Option<String>) -> Result<device::Device> {
    const RAZER_BLADE_16_2023_PID: u16 = 0x029f;
    let pid = match pid {
        Some(value) => match value {
            _ if value.starts_with("0x") => u16::from_str_radix(&value[2..], 16).unwrap(),
            _ => u16::from_str_radix(&value, 16).unwrap(),
        },
        None => RAZER_BLADE_16_2023_PID,
    };
    device::Device::new(pid)
}

#[derive(Parser)]
#[command(name = "razerctl", version, about)]
pub struct Razerctl {
    #[command(subcommand)]
    pub command: RazerCtlCommand,

    /// PID of the Razer device to use
    #[arg(short, long)]
    pub pid: Option<String>,
}

#[derive(Subcommand)]
pub enum RazerCtlCommand {
    /// List discovered Razer devices
    Enumerate,
    /// Control performance modes
    Perf(PerfModeCommand),
    /// Control fan
    Fan(FanCommand),
}

#[derive(Args)]
pub struct PerfModeCommand {
    #[command(subcommand)]
    pub action: PerfModeActionCommand,
}

#[derive(Subcommand)]
pub enum PerfModeActionCommand {
    /// Performance mode info
    Info,
    /// Set performance mode
    Mode { perf_mode: types::PerfMode },
    /// Set CPU boost
    Cpu { cpu_boost: types::CpuBoost },
    /// Set GPU boost
    Gpu { gpu_boost: types::GpuBoost },
}

#[derive(Args)]
pub struct FanCommand {
    #[command(subcommand)]
    pub subcommand: FanSubcommand,
}

#[derive(Subcommand)]
pub enum FanSubcommand {
    /// Set fan mode to auto
    Auto,
    /// Set fan mode to manual
    Manual,
    /// Set fan rpm
    Rpm {
        #[arg(value_parser = clap::value_parser!(u16).range(2000..=5000))]
        rpm: u16,
    },
    /// Control Max Fan Speed Mode
    Max {
        max_fan_speed_mode: types::MaxFanSpeedMode,
    },
}

fn main() -> Result<()> {
    let parser = Razerctl::parse();

    if let RazerCtlCommand::Enumerate = parser.command {
        return device::Device::enumerate();
    }

    let device = create_device(parser.pid)?;

    match parser.command {
        RazerCtlCommand::Enumerate => {
            unreachable!("Enumerate handled above")
        }
        RazerCtlCommand::Perf(command) => match command.action {
            PerfModeActionCommand::Info => command::print_info(&device),
            PerfModeActionCommand::Mode { perf_mode } => command::set_perf_mode(&device, perf_mode),
            PerfModeActionCommand::Cpu { cpu_boost } => command::set_cpu_boost(&device, cpu_boost),
            PerfModeActionCommand::Gpu { gpu_boost } => command::set_gpu_boost(&device, gpu_boost),
        },
        RazerCtlCommand::Fan(command) => match command.subcommand {
            FanSubcommand::Auto => command::set_fan_mode(&device, types::FanMode::Auto),
            FanSubcommand::Manual => command::set_fan_mode(&device, types::FanMode::Manual),
            FanSubcommand::Rpm { rpm } => command::set_fan_rpm(&device, rpm),
            FanSubcommand::Max { max_fan_speed_mode } => {
                command::set_max_fan_speed_mode(&device, max_fan_speed_mode)
            }
        },
    }
}
