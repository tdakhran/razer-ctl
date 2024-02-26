use librazer::command;
use librazer::device;
use librazer::types;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use clap_num::maybe_hex;

fn create_device(pid: Option<u16>) -> Result<device::Device> {
    const RAZER_BLADE_16_2023_PID: u16 = 0x029f;
    device::Device::new(pid.unwrap_or(RAZER_BLADE_16_2023_PID))
}

#[derive(Parser)]
#[command(name = "razerctl", version, about)]
struct Razerctl {
    #[command(subcommand)]
    pub command: RazerCtlCommand,

    /// PID of the Razer device to use
    #[clap(short, long, value_parser=maybe_hex::<u16>)]
    pub pid: Option<u16>,
}

#[derive(Subcommand)]
enum RazerCtlCommand {
    /// List discovered Razer devices
    Enumerate,
    /// Get device info
    Info,
    /// Control performance modes
    Perf(PerfModeCommand),
    /// Control fan
    Fan(FanCommand),
    /// Run Custom Command
    Cmd {
        #[clap(value_parser=maybe_hex::<u16>)]
        command: u16,
        #[clap(value_parser=maybe_hex::<u8>)]
        args: Vec<u8>,
    },
    /// Control Logo
    Logo { logo_mode: types::LogoMode },
}

#[derive(Args)]
struct PerfModeCommand {
    #[command(subcommand)]
    pub action: PerfModeActionCommand,
}

#[derive(Subcommand)]
enum PerfModeActionCommand {
    /// Set performance mode
    Mode { perf_mode: types::PerfMode },
    /// Set CPU boost
    Cpu { cpu_boost: types::CpuBoost },
    /// Set GPU boost
    Gpu { gpu_boost: types::GpuBoost },
}

#[derive(Args)]
struct FanCommand {
    #[command(subcommand)]
    pub subcommand: FanSubcommand,
}

#[derive(Subcommand)]
enum FanSubcommand {
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
        RazerCtlCommand::Info => Ok(println!("{}", command::get_info(&device)?)),
        RazerCtlCommand::Cmd { command, args } => command::custom_command(&device, command, &args),
        RazerCtlCommand::Perf(command) => match command.action {
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
        RazerCtlCommand::Logo { logo_mode } => command::set_logo_mode(&device, logo_mode),
    }
}
