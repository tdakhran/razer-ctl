use librazer::command;
use librazer::device;
use librazer::feature;
use librazer::types::{
    BatteryCare, CpuBoost, FanMode, FanZone, GpuBoost, LightsAlwaysOn, LogoMode, MaxFanSpeedMode,
    PerfMode,
};

use librazer::feature::Feature;

use anyhow::Result;
use clap::{arg, Command};

trait Cli: feature::Feature {
    fn cmd(&self) -> Option<Command> {
        None
    }
    fn handle(&self, _device: &device::Device, _matches: &clap::ArgMatches) -> Result<()> {
        Ok(())
    }
}

macro_rules! impl_unary_cmd_cli {
    ($parser:block, $name:literal, $arg_name:literal, $desc:literal,$arg_desc:literal) => {
        clap::Command::new($name)
            .about($desc)
            .arg(arg!(<$arg_name> $arg_desc).value_parser($parser))
            .arg_required_else_help(true)

    }
}

macro_rules! impl_unary_handle_cli {
    (<$arg_type:ty>($matches:ident, $device:ident, $name:literal, $arg_name:literal, $setter:path)) => {
        match $matches.subcommand() {
            Some(($name, matches)) => {
                $setter($device, *matches.get_one::<$arg_type>($arg_name).unwrap())?
            }
            _ => (),
        }
    };
}

macro_rules! impl_unary_cli {
    (<$feature_type:ty><$arg_type:ty>($desc:literal,$arg_desc:literal,$setter:path,$getter:path)) => {
        impl Cli for $feature_type {
            fn cmd(&self) -> Option<Command> {
                Some(
                    clap::Command::new(self.name())
                        .about($desc)
                        .arg(arg!(<ARG> $arg_desc).value_parser(clap::value_parser!($arg_type)))
                        .arg_required_else_help(true),
                )
            }
            fn handle(&self, device: &device::Device, matches: &clap::ArgMatches) -> Result<()> {
                match matches.subcommand() {
                    Some((ident, matches)) if ident == self.name() => {
                        let arg = matches.get_one::<$arg_type>("ARG").unwrap();
                        $setter(device, *arg)
                    }
                    Some(("info", _)) => Ok(println!("{}: {:?}", self.name(), $getter(device))),
                    _ => Ok(()),
                }
            }
        }

    }
}

impl_unary_cli! {<feature::KbdBacklight><u8>("Set keyboard backlight brightness", "Number in range [0, 255]", command::set_keyboard_brightness, command::get_keyboard_brightness)}
impl_unary_cli! {<feature::BatteryCare><BatteryCare>("Enable or disable battery care", "", command::set_battery_care, command::get_battery_care)}
impl_unary_cli! {<feature::LidLogo><LogoMode>("Set lid logo mode", "", command::set_logo_mode, command::get_logo_mode)}
impl_unary_cli! {<feature::LightsAlwaysOn><LightsAlwaysOn>("Set lights always on", "", command::set_lights_always_on, command::get_lights_always_on)}

struct CustomCommand;

impl Feature for CustomCommand {
    fn name(&self) -> &'static str {
        "cmd"
    }
}

impl Cli for CustomCommand {
    fn cmd(&self) -> Option<Command> {
        Some(
            clap::Command::new(self.name())
                .about("Run custom command [WARNING: Use at your own risk]")
                .arg(
                    arg!(<COMMAND> "Command in hex format, e.g. 0x0d82")
                        .required(true)
                        .value_parser(clap_num::maybe_hex::<u16>),
                )
                .arg(
                    arg!(<ARGS>... "Arguments to the command, e.g. 0 1 3 5")
                        .required(false)
                        .trailing_var_arg(true)
                        .value_parser(clap_num::maybe_hex::<u8>),
                )
                .arg_required_else_help(true),
        )
    }
    fn handle(&self, device: &device::Device, matches: &clap::ArgMatches) -> Result<()> {
        match matches.subcommand() {
            Some((ident, matches)) if ident == self.name() => {
                let cmd = *matches.get_one::<u16>("COMMAND").unwrap();
                let args: Vec<u8> = matches.get_many::<u8>("ARGS").unwrap().copied().collect();
                println!("Running custom command: {:x?} {:?}", cmd, args);
                command::custom_command(device, cmd, &args)
            }
            _ => Ok(()),
        }
    }
}

impl Cli for feature::Fan {
    fn cmd(&self) -> Option<Command> {
        Some(
            clap::Command::new(self.name())
                .about("Control fan")
                .subcommand(clap::Command::new("auto").about("Set fan mode to auto"))
                .subcommand(clap::Command::new("manual").about("Set fan mode to manual"))
                .subcommand(impl_unary_cmd_cli!{{clap::value_parser!(u16).range(2000..=5000)}, "rpm", "RPM", "Set fan rpm", "Fan RPM in range [2000, 5000]"})
                .subcommand(impl_unary_cmd_cli!{{clap::value_parser!(MaxFanSpeedMode)}, "max", "MAX", "Control Max Fan Speed Mode", "Max Fan Speed Mode"})
                .arg_required_else_help(true),
        )
    }

    fn handle(&self, device: &device::Device, matches: &clap::ArgMatches) -> Result<()> {
        match matches.subcommand() {
            Some((ident, matches)) if ident == self.name() => {
                impl_unary_handle_cli! {<u16>(matches, device, "rpm", "RPM", command::set_fan_rpm)}
                impl_unary_handle_cli! {<MaxFanSpeedMode>(matches, device, "max", "MAX", command::set_max_fan_speed_mode)}

                match matches.subcommand() {
                    Some(("auto", _)) => command::set_fan_mode(device, FanMode::Auto),
                    Some(("manual", _)) => command::set_fan_mode(device, FanMode::Manual),
                    _ => Ok(()),
                }
            }
            Some(("info", _)) => {
                match command::get_perf_mode(device) {
                    Ok((_, fan_mode @ FanMode::Auto)) => println!("Fan: {:?}", fan_mode),
                    Ok((_, fan_mode @ FanMode::Manual)) => {
                        println!(
                            "Fan: {:?}@{:?} RPM",
                            fan_mode,
                            command::get_fan_rpm(device, FanZone::Zone1)
                        )
                    }
                    Err(e) => println!("{}", e),
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

impl Cli for feature::Perf {
    fn cmd(&self) -> Option<Command> {
        Some(
            clap::Command::new(self.name())
                .about("Control performance modes")
                .subcommand(impl_unary_cmd_cli!{{clap::value_parser!(PerfMode)}, "mode", "MODE", "Set performance mode", "Performance mode"})
                .subcommand(impl_unary_cmd_cli!{{clap::value_parser!(CpuBoost)}, "cpu", "CPU", "Set CPU boost", "CPU boost"})
                .subcommand( impl_unary_cmd_cli!{{clap::value_parser!(GpuBoost)}, "gpu", "GPU", "Set GPU boost", "GPU boost"})
                .arg_required_else_help(true),
        )
    }

    fn handle(&self, device: &device::Device, matches: &clap::ArgMatches) -> Result<()> {
        match matches.subcommand() {
            Some((ident, matches)) if ident == self.name() => {
                impl_unary_handle_cli! {<PerfMode>(matches, device, "mode", "MODE", command::set_perf_mode)}
                impl_unary_handle_cli! {<CpuBoost>(matches, device, "cpu", "CPU", command::set_cpu_boost)}
                impl_unary_handle_cli! {<GpuBoost>(matches, device, "gpu", "GPU", command::set_gpu_boost)}
                Ok(())
            }
            Some(("info", _)) => {
                let perf_mode = command::get_perf_mode(device);
                println!("Performance: {:?}", perf_mode);
                if let Ok((PerfMode::Custom, _)) = perf_mode {
                    let cpu_boost = command::get_cpu_boost(device);
                    let gpu_boost = command::get_gpu_boost(device);
                    println!("CPU: {:?}", cpu_boost);
                    println!("GPU: {:?}", gpu_boost);

                    if let (Ok(CpuBoost::Boost) | Ok(CpuBoost::Overclock), Ok(GpuBoost::High)) =
                        (cpu_boost, gpu_boost)
                    {
                        println!(
                            "Max Fan Speed: {:?}",
                            command::get_max_fan_speed_mode(device)
                        )
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

fn enumerate() -> Result<()> {
    let (pid_list, model_number_prefix) = device::Device::enumerate()?;

    println!("Model: {}", model_number_prefix);
    println!(
        "Supported: {}",
        device::SUPPORTED
            .iter()
            .any(|supported| model_number_prefix == supported.model_number_prefix)
    );
    println!("PID: {:#06x?}", pid_list);
    Ok(())
}

fn update_cmd(cmd: Command, features: &[Box<dyn Cli>]) -> Command {
    features
        .iter()
        .filter_map(|f| f.cmd())
        .fold(cmd, |cmd, f| cmd.subcommand(f))
}

fn handle(
    device: &device::Device,
    matches: &clap::ArgMatches,
    features: &Vec<Box<dyn Cli>>,
) -> Result<()> {
    if let Some(("info", _)) = matches.subcommand() {
        println!("Device: {:?}", device.info);
    }

    for f in features {
        f.handle(device, matches)?;
    }
    Ok(())
}

fn gen_cli_features(feature_list: &[&str]) -> Vec<Box<dyn Cli>> {
    use feature::*;
    librazer::iter_features!(|_, feature| -> Box<dyn Cli> { Box::new(feature) })
        .into_iter()
        .filter(|f| feature_list.contains(&f.name()))
        .collect()
}

fn main() -> Result<()> {
    let info_cmd = clap::Command::new("info").about("Get device info");
    let auto_cmd = clap::Command::new("auto")
        .about("Automatically detect supported Razer device and enable device specific features")
        .subcommand(info_cmd.clone())
        .subcommand_required(true);

    let manual_cmd =clap::Command::new("manual").about("Manually specify PID of the Razer device and enable all features (many might not work)")
            .arg(
                arg!(-p --pid <PID> "PID of the Razer device to use")
                .required(true)
                .value_parser(clap_num::maybe_hex::<u16>)
            )
            .arg_required_else_help(true)
            .subcommand(info_cmd)
            .subcommand_required(true);

    // TODO: find a better way to detect auto mode in advance
    let is_auto_mode = std::env::args_os().nth(1) == Some("auto".into());
    let device = is_auto_mode.then_some(device::Device::detect()?);
    let feature_list = match device {
        Some(ref device) => device.info.features,
        _ => feature::ALL_FEATURES,
    };

    let mut cli_features: Vec<Box<dyn Cli>> = gen_cli_features(feature_list);
    cli_features.push(Box::new(CustomCommand));

    let cmd = clap::command!()
        .color(clap::ColorChoice::Always)
        .subcommand_required(true)
        .subcommand(update_cmd(auto_cmd, &cli_features))
        .subcommand(update_cmd(manual_cmd, &cli_features))
        .subcommand(clap::Command::new("enumerate").about("List discovered Razer devices"));

    let matches = cmd.get_matches();

    match matches.subcommand() {
        Some(("enumerate", _)) => {
            enumerate()?;
        }
        Some(("auto", submatches)) => {
            handle(&device.unwrap(), submatches, &cli_features)?;
        }
        Some(("manual", submatches)) => {
            let device = device::Device::new(device::Descriptor {
                model_number_prefix: "Unknown",
                name: "Unknown",
                pid: *submatches.get_one::<u16>("pid").unwrap(),
                features: feature::ALL_FEATURES,
            })?;
            handle(&device, submatches, &cli_features)?;
        }
        Some((cmd, _)) => unimplemented!("Subcommand not implemented: {}", cmd),
        None => unreachable!(),
    };

    Ok(())
}
