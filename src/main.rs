mod razer {
    mod device;
    mod packet;

    use packet::Packet;

    use anyhow::{anyhow, Result};
    use clap::ValueEnum;

    const RAZER_BLADE_16_2023_PID: u16 = 0x029f;

    // commands to implement
    // 01 070f - ???
    // 03 0303 - set kbd backlight brightness | 0x01 0x05 [0x00;0xff]
    // 03 0383 - get kbd backlight brightness | 0x01 0x05 0x00 | arg2 of response is result
    // 03 0d01 - set rpm | 0x00 [0x01;0x02] (rpm/100) as u8
    // 03 0d07 - set boost 0x00 0x01 [CPU (0 - low, 1 - medium, 2 - high, 3 - boost, 4 - overclock)]
    //                               [GPU (0 - low, 1 - medium, 2 - high)]
    // 03 0d87 - get boost 0x00 [0x01 - cpu, 0x02 - gpu] | arg2 of response is result
    // 04 0d02 - set power mode |  0x00 [0x01 - cpu;0x02 - gpu]  power-mode         fan-auto?
    //                                                           00 - balanced      00 - auto
    //                                                           05 - silent        01 - manual
    //                                                           04 - custom
    // 04 0d82 - get power mode | 0x00 [0x01 - cpu;0x02 - gpu] 0x00 0x00 | arg2 of response is result

    #[derive(Clone, Debug)]
    pub enum FanMode {
        Auto,
        Manual(u16),
    }

    impl From<FanMode> for u8 {
        fn from(mode: FanMode) -> u8 {
            match mode {
                FanMode::Auto => 0x00,
                FanMode::Manual(_) => 0x01,
            }
        }
    }

    #[derive(Clone, Debug, ValueEnum)]
    pub enum CpuBoost {
        Low = 0,
        Medium = 1,
        High = 2,
        Boost = 3,
        Overclock = 4,
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
                _ => Err(anyhow!("Failed to convert {} to CpuBoost", value)),
            }
        }
    }

    #[derive(Clone, Debug, ValueEnum)]
    pub enum GpuBoost {
        Low = 0,
        Medium = 1,
        High = 2,
    }

    impl TryFrom<u8> for GpuBoost {
        type Error = anyhow::Error;

        fn try_from(value: u8) -> Result<Self, Self::Error> {
            match value {
                0 => Ok(Self::Low),
                1 => Ok(Self::Medium),
                2 => Ok(Self::High),
                _ => Err(anyhow!("Failed to convert {} to GpuBoost", value)),
            }
        }
    }

    #[repr(u8)]
    #[derive(Clone, Debug)]
    pub enum PowerMode {
        Balanced,
        Silent,
        Custom(CpuBoost, GpuBoost),
    }

    impl From<PowerMode> for u8 {
        fn from(mode: PowerMode) -> u8 {
            match mode {
                PowerMode::Balanced => 0x00,
                PowerMode::Silent => 0x05,
                PowerMode::Custom(_, _) => 0x04,
            }
        }
    }

    #[derive(Clone)]
    pub enum Zone {
        Cpu = 0x01,
        Gpu = 0x02,
    }

    pub struct Device0 {
        device: device::Device,
    }

    impl Device0 {
        pub fn new(pid: Option<String>) -> Device0 {
            let pid = match pid {
                Some(value) => match value {
                    _ if value.starts_with("0x") => u16::from_str_radix(&value[2..], 16).unwrap(),
                    _ => u16::from_str_radix(&value, 16).unwrap(),
                },
                None => RAZER_BLADE_16_2023_PID,
            };
            let device = device::Device::new(pid).unwrap();
            Device0 { device }
        }

        fn send_report(&mut self, report: Packet) -> Result<Packet> {
            self.device.send(report)
        }

        fn _get_power_mode_command(&mut self, zone: u8) -> Result<u8> {
            let mut report: Packet = Packet::new(0x0d, 0x82, 0x04);
            assert!(zone == 0x01 || zone == 0x02);
            report.set_args(&[0x00, zone]);
            Ok(self.send_report(report)?.get_args()[2])
        }

        fn _get_boost_command(&mut self, zone: u8) -> Result<u8> {
            let mut report: Packet = Packet::new(0x0d, 0x87, 0x03);
            assert!(zone == 0x01 || zone == 0x02);
            report.set_args(&[0x00, zone]);
            Ok(self.send_report(report)?.get_args()[2])
        }

        pub fn get_power_mode(&mut self) -> Result<PowerMode> {
            let cpu_power_mode = self._get_power_mode_command(Zone::Cpu as u8)?;
            let gpu_power_mode = self._get_power_mode_command(Zone::Gpu as u8)?;

            if cpu_power_mode != gpu_power_mode {
                return Err(anyhow!("CPU and GPU power modes are different"));
            }

            match cpu_power_mode {
                0x00 => Ok(PowerMode::Balanced),
                0x05 => Ok(PowerMode::Silent),
                0x04 => Ok(PowerMode::Custom(
                    CpuBoost::try_from(self._get_boost_command(Zone::Cpu as u8)?)?,
                    GpuBoost::try_from(self._get_boost_command(Zone::Gpu as u8)?)?,
                )),
                _ => Err(anyhow!("Unknown power mode {}", cpu_power_mode)),
            }
        }

        fn _set_power_mode_command(
            &mut self,
            zone: u8,
            power_mode: u8,
            fan_mode: u8,
        ) -> Result<()> {
            assert!(zone == 0x01 || zone == 0x02);
            assert!([0x00, 0x05, 0x04].contains(&power_mode));

            let mut report: Packet = Packet::new(0x0d, 0x02, 0x04);
            report.set_args(&[0x00, zone, power_mode, fan_mode]);
            self.send_report(report).map(|_| ())
        }

        fn _set_boost_command(&mut self, zone: u8, boost: u8) -> Result<()> {
            let mut report: Packet = Packet::new(0x0d, 0x07, 0x03);
            report.set_args(&[0x00, zone, boost]);
            self.send_report(report).map(|_| ())
        }

        pub fn set_power_mode(&mut self, power_mode: PowerMode, fan_mode: FanMode) -> Result<()> {
            for zone in [Zone::Cpu as u8, Zone::Gpu as u8].into_iter() {
                self._get_power_mode_command(zone)?;
                self._set_power_mode_command(
                    zone,
                    From::from(power_mode.clone()),
                    From::from(fan_mode.clone()),
                )?;
            }

            if let PowerMode::Custom(cpu_boost, gpu_boost) = power_mode {
                self._get_boost_command(Zone::Cpu as u8)?;
                self._set_boost_command(Zone::Cpu as u8, cpu_boost as u8)?;
                self._get_boost_command(Zone::Gpu as u8)?;
                self._set_boost_command(Zone::Gpu as u8, gpu_boost as u8)?;
            }

            Ok(())
        }

        fn _set_rpm_command(&mut self, zone: u8, rpm: u8) -> Result<()> {
            let mut report: Packet = Packet::new(0x0d, 0x01, 0x03);
            report.set_args(&[0x00, zone, rpm]);
            self.send_report(report).map(|_| ())
        }

        pub fn set_fan_mode(&mut self, fan_mode: FanMode) -> Result<()> {
            match fan_mode {
                FanMode::Manual(rpm) => {
                    for zone in [Zone::Cpu, Zone::Gpu].iter() {
                        let power_mode = self._get_power_mode_command(Zone::Cpu as u8)?;
                        if power_mode == From::from(PowerMode::Custom(CpuBoost::Low, GpuBoost::Low))
                        {
                            return Err(anyhow!("Fan speed can not be set in custom power mode"));
                        }
                        self._set_power_mode_command(
                            zone.clone() as u8,
                            power_mode,
                            From::from(FanMode::Manual(rpm)),
                        )?;
                        self._set_rpm_command(zone.clone() as u8, (rpm / 100) as u8)?;
                    }
                }
                FanMode::Auto => {
                    for zone in [Zone::Cpu, Zone::Gpu].iter() {
                        let power_mode = self._get_power_mode_command(Zone::Cpu as u8)?;
                        self._set_power_mode_command(
                            zone.clone() as u8,
                            power_mode,
                            From::from(FanMode::Auto),
                        )?;
                    }
                }
            }
            Ok(())
        }

        pub fn set_undocumented(&mut self, value: u8) -> Result<()> {
            let mut report: Packet = Packet::new(0x07, 0x0f, 0x01);
            report.set_args(&[value]);
            self.send_report(report).map(|_| ())
        }
    }

    pub fn enumerate() -> Result<()> {
        device::Device::enumerate()
    }
}

use anyhow::Result;
use clap::{Args, Parser, Subcommand};

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
    /// List all Razer devices
    Enumerate,
    /// Get or set power mode
    Power(PowerCommand),
    /// Set fan speed
    Fan(FanCommand),
    /// Undocumented command
    Undocumented {
        #[arg(short, long, value_parser = clap::value_parser!(u8).range(0..=2))]
        value: u8,
    },
}

#[derive(Args)]
pub struct PowerCommand {
    #[command(subcommand)]
    pub subcommand: PowerSubcommand,
}

#[derive(Subcommand)]
pub enum PowerSubcommand {
    /// Get power mode
    Get,
    /// Set power mode
    Set(PowerSetSubcommand),
}

#[derive(Args)]
pub struct PowerSetSubcommand {
    #[command(subcommand)]
    pub subcommand: PowerSetSubcommandArgs,
}

#[derive(Subcommand)]
pub enum PowerSetSubcommandArgs {
    /// Balanced power mode
    Balanced,
    /// Silent power mode
    Silent,
    /// Custom power mode
    Custom {
        cpu_boost: razer::CpuBoost,
        gpu_boost: razer::GpuBoost,
    },
}

#[derive(Args)]
pub struct FanCommand {
    #[command(subcommand)]
    pub subcommand: FanSubcommand,
}

#[derive(Subcommand)]
pub enum FanSubcommand {
    /// Set fan speed to auto
    Auto,
    /// Set fan speed to rpm
    Set {
        #[arg(short, long, value_parser = clap::value_parser!(u16).range(2000..=5000))]
        rpm: u16,
    },
}

fn main() -> Result<()> {
    let parser = Razerctl::parse();

    let mut laptop = razer::Device0::new(parser.pid);

    match parser.command {
        RazerCtlCommand::Enumerate => razer::enumerate()?,

        RazerCtlCommand::Power(command) => match command.subcommand {
            PowerSubcommand::Get => {
                println!("{:#?}", laptop.get_power_mode()?);
            }
            PowerSubcommand::Set(subcommand) => match subcommand.subcommand {
                PowerSetSubcommandArgs::Balanced => {
                    laptop.set_power_mode(razer::PowerMode::Balanced, razer::FanMode::Auto)?
                }
                PowerSetSubcommandArgs::Silent => {
                    laptop.set_power_mode(razer::PowerMode::Silent, razer::FanMode::Auto)?
                }
                PowerSetSubcommandArgs::Custom {
                    cpu_boost,
                    gpu_boost,
                } => laptop.set_power_mode(
                    razer::PowerMode::Custom(cpu_boost, gpu_boost),
                    razer::FanMode::Auto,
                )?,
            },
        },
        RazerCtlCommand::Fan(command) => match command.subcommand {
            FanSubcommand::Set { rpm } => laptop.set_fan_mode(razer::FanMode::Manual(rpm))?,
            FanSubcommand::Auto => laptop.set_fan_mode(razer::FanMode::Auto)?,
        },
        RazerCtlCommand::Undocumented { value } => laptop.set_undocumented(value)?,
    }

    Ok(())
}
