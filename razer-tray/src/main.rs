#![windows_subsystem = "windows"]
use anyhow::Result;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

use librazer::types::{BatteryCare, CpuBoost, GpuBoost, LightsAlwaysOn, LogoMode, MaxFanSpeedMode};
use librazer::{command, device};

use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::{
    menu::{CheckMenuItem, IsMenuItem, Menu, MenuEvent, PredefinedMenuItem, Submenu},
    TrayIconBuilder, TrayIconEvent,
};

const PKG_NAME: &str = env!("CARGO_PKG_NAME");

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
enum FanSpeed {
    Auto,
    Manual(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
enum PerfMode {
    Silent,
    Balanced(FanSpeed),
    Custom(CpuBoost, GpuBoost, MaxFanSpeedMode),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct LightsMode {
    logo_mode: LogoMode,
    keyboard_brightness: u8,
    always_on: LightsAlwaysOn,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct DeviceState {
    perf_mode: PerfMode,
    lights_mode: LightsMode,
    battery_care: BatteryCare,
}

impl DeviceState {
    fn read(device: &device::Device) -> Result<Self> {
        let perf_mode = match command::get_perf_mode(device)? {
            (librazer::types::PerfMode::Silent, _) => PerfMode::Silent,
            (librazer::types::PerfMode::Balanced, librazer::types::FanMode::Auto) => {
                PerfMode::Balanced(FanSpeed::Auto)
            }
            (librazer::types::PerfMode::Balanced, librazer::types::FanMode::Manual) => {
                let fan_speed = command::get_fan_rpm(device, librazer::types::FanZone::Zone1)?;
                PerfMode::Balanced(FanSpeed::Manual(fan_speed))
            }
            (librazer::types::PerfMode::Custom, _) => {
                let cpu_boost = command::get_cpu_boost(device)?;
                let gpu_boost = command::get_gpu_boost(device)?;
                let max_fan_speed = command::get_max_fan_speed_mode(device)?;
                PerfMode::Custom(cpu_boost, gpu_boost, max_fan_speed)
            }
        };

        let lights_mode = LightsMode {
            logo_mode: command::get_logo_mode(device)?,
            keyboard_brightness: command::get_keyboard_brightness(device)?,
            always_on: command::get_lights_always_on(device)?,
        };

        let battery_care = command::get_battery_care(device)?;

        Ok(Self {
            perf_mode,
            lights_mode,
            battery_care,
        })
    }

    fn apply(&self, device: &device::Device) -> Result<()> {
        match self.perf_mode {
            PerfMode::Silent => command::set_perf_mode(device, librazer::types::PerfMode::Silent),
            PerfMode::Balanced(FanSpeed::Auto) => {
                command::set_perf_mode(device, librazer::types::PerfMode::Balanced)
            }
            PerfMode::Balanced(FanSpeed::Manual(rpm)) => {
                command::set_perf_mode(device, librazer::types::PerfMode::Balanced)?;
                command::set_fan_mode(device, librazer::types::FanMode::Manual)?;
                command::set_fan_rpm(device, rpm)
            }
            PerfMode::Custom(cpu_boost, gpu_boost, max_fan_speed) => {
                command::set_perf_mode(device, librazer::types::PerfMode::Custom)?;
                command::set_cpu_boost(device, cpu_boost)?;
                command::set_gpu_boost(device, gpu_boost)?;
                command::set_max_fan_speed_mode(device, max_fan_speed)
            }
        }?;

        match self.lights_mode.logo_mode {
            LogoMode::Static => command::set_logo_mode(device, LogoMode::Static),
            LogoMode::Breathing => command::set_logo_mode(device, LogoMode::Breathing),
            LogoMode::Off => command::set_logo_mode(device, LogoMode::Off),
        }?;

        command::set_keyboard_brightness(device, self.lights_mode.keyboard_brightness)?;
        command::set_lights_always_on(device, self.lights_mode.always_on)?;
        command::set_battery_care(device, self.battery_care)
    }

    fn perf_delta(
        &self,
        cpu_boost: Option<CpuBoost>,
        gpu_boost: Option<GpuBoost>,
        max_fan_speed: Option<MaxFanSpeedMode>,
    ) -> Self {
        DeviceState {
            perf_mode: if let PerfMode::Custom(cb, gb, mfs) = self.perf_mode {
                PerfMode::Custom(
                    cpu_boost.unwrap_or(cb),
                    gpu_boost.unwrap_or(gb),
                    max_fan_speed.unwrap_or(mfs),
                )
            } else {
                PerfMode::Custom(
                    cpu_boost.unwrap_or(CpuBoost::Boost),
                    gpu_boost.unwrap_or(GpuBoost::High),
                    max_fan_speed.unwrap_or(MaxFanSpeedMode::Disable),
                )
            },
            ..*self
        }
    }
}

impl Default for DeviceState {
    fn default() -> Self {
        Self {
            perf_mode: PerfMode::Balanced(FanSpeed::Auto),
            lights_mode: LightsMode {
                logo_mode: LogoMode::Off,
                keyboard_brightness: 0,
                always_on: LightsAlwaysOn::Disable,
            },
            battery_care: BatteryCare::Disable,
        }
    }
}

trait DeviceStateDelta<T> {
    fn delta(&self, property: T) -> Self;
}

impl DeviceStateDelta<CpuBoost> for DeviceState {
    fn delta(&self, cpu_boost: CpuBoost) -> Self {
        self.perf_delta(Some(cpu_boost), None, None)
    }
}

impl DeviceStateDelta<GpuBoost> for DeviceState {
    fn delta(&self, gpu_boost: GpuBoost) -> Self {
        self.perf_delta(None, Some(gpu_boost), None)
    }
}

impl DeviceStateDelta<MaxFanSpeedMode> for DeviceState {
    fn delta(&self, max_fan_speed_mode: MaxFanSpeedMode) -> Self {
        self.perf_delta(None, None, Some(max_fan_speed_mode))
    }
}

struct ProgramState {
    device_state: DeviceState,
    event_handlers: std::collections::HashMap<String, DeviceState>,
    menu: Menu,
}

impl ProgramState {
    fn new(device_state: DeviceState) -> Result<Self> {
        let (menu, event_handlers) = Self::create_menu_and_handlers(&device_state)?;
        Ok(Self {
            device_state,
            event_handlers,
            menu,
        })
    }

    fn create_menu_and_handlers(
        dstate: &DeviceState,
    ) -> Result<(Menu, std::collections::HashMap<String, DeviceState>)> {
        let mut event_handlers = std::collections::HashMap::new();
        let menu = Menu::new();
        // header

        // perf
        let perf_modes = Submenu::new("Performance", true);
        // silent
        perf_modes.append(&CheckMenuItem::with_id(
            format!("{:?}", PerfMode::Silent),
            "Silent",
            dstate.perf_mode != PerfMode::Silent,
            dstate.perf_mode == PerfMode::Silent,
            None,
        ))?;
        event_handlers.insert(
            format!("{:?}", PerfMode::Silent),
            DeviceState {
                perf_mode: PerfMode::Silent,
                ..*dstate
            },
        );
        // balanced
        let fan_speeds: Vec<CheckMenuItem> = [CheckMenuItem::with_id(
            "fan_speed:auto",
            "Fan: Auto",
            dstate.perf_mode != PerfMode::Balanced(FanSpeed::Auto),
            dstate.perf_mode == PerfMode::Balanced(FanSpeed::Auto),
            None,
        )]
        .into_iter()
        .chain((2000..=5000).step_by(500).map(|rpm| {
            let event_id = format!("fan_speed:{}", rpm);
            event_handlers.insert(
                event_id.clone(),
                DeviceState {
                    perf_mode: PerfMode::Balanced(FanSpeed::Manual(rpm)),
                    ..*dstate
                },
            );
            CheckMenuItem::with_id(
                event_id,
                format!("Fan: {} RPM", rpm),
                dstate.perf_mode != PerfMode::Balanced(FanSpeed::Manual(rpm)),
                dstate.perf_mode == PerfMode::Balanced(FanSpeed::Manual(rpm)),
                None,
            )
        }))
        .collect();
        event_handlers.insert(
            "fan_speed:auto".to_string(),
            DeviceState {
                perf_mode: PerfMode::Balanced(FanSpeed::Auto),
                ..*dstate
            },
        );

        perf_modes.append(&Submenu::with_items(
            "Balanced",
            true,
            &fan_speeds
                .iter()
                .map(|i| i as &dyn IsMenuItem)
                .collect::<Vec<_>>(),
        )?)?;

        // custom
        let cpu_boosts: Vec<CheckMenuItem> = CpuBoost::iter()
            .map(|boost| {
                let event_id = format!("cpu_boost:{:?}", boost);
                event_handlers.insert(event_id.clone(), dstate.delta(boost));
                let checked = matches!(dstate.perf_mode, PerfMode::Custom(b, _, _) if b == boost);
                CheckMenuItem::with_id(event_id, format!("{:?}", boost), !checked, checked, None)
            })
            .collect();

        let gpu_boosts: Vec<CheckMenuItem> = GpuBoost::iter()
            .map(|boost| {
                let event_id = format!("gpu_boost:{:?}", boost);
                event_handlers.insert(event_id.clone(), dstate.delta(boost));
                let checked = matches!(dstate.perf_mode, PerfMode::Custom(_, b, _) if b == boost);
                CheckMenuItem::with_id(event_id, format!("{:?}", boost), !checked, checked, None)
            })
            .collect();

        let max_fan_speed_mode = &[CheckMenuItem::with_id(
            "max_fan_speed_mode",
            "Max Fan Speed",
            true,
            matches!(
                dstate.perf_mode,
                PerfMode::Custom(_, _, MaxFanSpeedMode::Enable)
            ),
            None,
        )];
        event_handlers.insert(
            "max_fan_speed_mode".to_string(),
            match dstate.perf_mode {
                PerfMode::Custom(_, _, MaxFanSpeedMode::Enable) => {
                    dstate.delta(MaxFanSpeedMode::Disable)
                }
                _ => dstate.delta(MaxFanSpeedMode::Enable),
            },
        );

        let separator = PredefinedMenuItem::separator();

        perf_modes.append(&Submenu::with_items(
            "Custom",
            true,
            &cpu_boosts
                .iter()
                .map(|i| i as &dyn IsMenuItem)
                .chain([&separator as &dyn IsMenuItem])
                .chain(gpu_boosts.iter().map(|i| i as &dyn IsMenuItem))
                .chain([&separator as &dyn IsMenuItem])
                .chain(max_fan_speed_mode.iter().map(|i| i as &dyn IsMenuItem))
                .collect::<Vec<_>>(),
        )?)?;

        menu.append(&perf_modes)?;

        // logo
        menu.append(&PredefinedMenuItem::separator())?;
        let modes = LogoMode::iter()
            .map(|mode| {
                let event_id = format!("logo_mode:{:?}", mode);
                event_handlers.insert(
                    event_id.clone(),
                    DeviceState {
                        lights_mode: LightsMode {
                            logo_mode: mode,
                            ..dstate.lights_mode
                        },
                        ..*dstate
                    },
                );
                CheckMenuItem::with_id(
                    event_id,
                    format!("{:?}", mode),
                    dstate.lights_mode.logo_mode != mode,
                    dstate.lights_mode.logo_mode == mode,
                    None,
                )
            })
            .collect::<Vec<_>>();

        menu.append(&Submenu::with_items(
            "Logo",
            true,
            &modes
                .iter()
                .map(|i| i as &dyn IsMenuItem)
                .collect::<Vec<_>>(),
        )?)?;
        menu.append(&PredefinedMenuItem::separator())?;

        // lights always on
        menu.append(&CheckMenuItem::with_id(
            "lights_always_on",
            "Lights always on",
            true,
            dstate.lights_mode.always_on == LightsAlwaysOn::Enable,
            None,
        ))?;
        event_handlers.insert(
            "lights_always_on".to_string(),
            DeviceState {
                lights_mode: LightsMode {
                    always_on: match dstate.lights_mode.always_on {
                        LightsAlwaysOn::Enable => LightsAlwaysOn::Disable,
                        LightsAlwaysOn::Disable => LightsAlwaysOn::Enable,
                    },
                    ..dstate.lights_mode
                },
                ..*dstate
            },
        );

        let brightness_modes: Vec<CheckMenuItem> = (0..=100)
            .step_by(10)
            .map(|brightness| {
                let event_id = format!("brightness:{}", brightness);
                event_handlers.insert(
                    event_id.clone(),
                    DeviceState {
                        lights_mode: LightsMode {
                            keyboard_brightness: brightness / 2 * 5,
                            ..dstate.lights_mode
                        },
                        ..*dstate
                    },
                );
                CheckMenuItem::with_id(
                    event_id,
                    format!("Brightness: {}", brightness),
                    dstate.lights_mode.keyboard_brightness != brightness / 2 * 5,
                    dstate.lights_mode.keyboard_brightness == brightness / 2 * 5,
                    None,
                )
            })
            .collect();

        menu.append(&Submenu::with_items(
            "Brightness",
            true,
            &brightness_modes
                .iter()
                .map(|i| i as &dyn IsMenuItem)
                .collect::<Vec<_>>(),
        )?)?;

        // battery health optimizer
        menu.append_items(&[
            &PredefinedMenuItem::separator(),
            &CheckMenuItem::with_id(
                "bho",
                "Battery Health Optimizer",
                true,
                dstate.battery_care == BatteryCare::Enable,
                None,
            ),
        ])?;
        event_handlers.insert(
            "bho".to_string(),
            DeviceState {
                battery_care: match dstate.battery_care {
                    BatteryCare::Enable => BatteryCare::Disable,
                    BatteryCare::Disable => BatteryCare::Enable,
                },
                ..*dstate
            },
        );

        // footer
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&PredefinedMenuItem::about(None, Some(Self::about())))?;
        menu.append(&PredefinedMenuItem::quit(None))?;

        Ok((menu, event_handlers))
    }

    fn handle_event(&self, event_id: &str) -> Result<DeviceState> {
        let next_state = self.event_handlers.get(event_id).ok_or(anyhow::anyhow!(
            "No event handler found for event_id: {}",
            event_id
        ))?;
        Ok(*next_state)
    }

    fn about() -> tray_icon::menu::AboutMetadata {
        tray_icon::menu::AboutMetadata {
            name: Some(PKG_NAME.into()),
            version: Some(env!("CARGO_PKG_VERSION").into()),
            authors: Some(
                env!("CARGO_PKG_AUTHORS")
                    .split(';')
                    .map(|a| a.trim().to_string())
                    .collect::<Vec<_>>(),
            ),
            website: Some(format!(
                "{}\nLog: {}",
                env!("CARGO_PKG_HOMEPAGE"),
                get_logging_file_path().display()
            )),
            comments: Some(env!("CARGO_PKG_DESCRIPTION").into()),
            ..Default::default()
        }
    }

    fn get_next_perf_mode(&self) -> DeviceState {
        DeviceState {
            perf_mode: match self.device_state.perf_mode {
                PerfMode::Silent => PerfMode::Balanced(FanSpeed::Auto),
                PerfMode::Balanced(..) => {
                    PerfMode::Custom(CpuBoost::Boost, GpuBoost::High, MaxFanSpeedMode::Disable)
                }
                PerfMode::Custom(..) => PerfMode::Silent,
            },
            ..self.device_state
        }
    }

    fn tooltip(&self) -> Result<String> {
        use std::fmt::Write;
        let mut info = String::new();
        let mut status = String::new();

        match self.device_state.perf_mode {
            PerfMode::Silent => writeln!(&mut info, "Silent")?,
            PerfMode::Balanced(FanSpeed::Auto) => {
                writeln!(&mut info, "Balanced (Auto)")?;
            }
            PerfMode::Balanced(FanSpeed::Manual(rpm)) => {
                writeln!(&mut info, "Balanced {}", rpm)?;
            }
            PerfMode::Custom(cpu_boost, gpu_boost, max_fan_speed) => {
                writeln!(&mut info, "Custom",)?;
                if max_fan_speed == MaxFanSpeedMode::Enable {
                    status.push('💨');
                }
                writeln!(&mut info, "CPU: {:?}", cpu_boost)?;
                writeln!(&mut info, "GPU: {:?}", gpu_boost)?;
            }
        }

        writeln!(
            &mut info,
            "Logo: {:?}",
            self.device_state.lights_mode.logo_mode
        )?;

        if self.device_state.lights_mode.keyboard_brightness > 0 {
            writeln!(
                &mut info,
                "🔆: {:?}",
                self.device_state.lights_mode.keyboard_brightness
            )?;
        }

        if self.device_state.lights_mode.always_on == LightsAlwaysOn::Enable {
            status.push('💡');
        }

        if self.device_state.battery_care == BatteryCare::Enable {
            status.push('🔋');
        }

        Ok((info.to_string() + &status).trim_end().to_string())
    }

    fn icon(&self) -> tray_icon::Icon {
        let razer_red = include_bytes!("../icons/razer-red.png");
        let razer_yellow = include_bytes!("../icons/razer-yellow.png");
        let razer_green = include_bytes!("../icons/razer-green.png");

        let image = match self.device_state.perf_mode {
            PerfMode::Silent => image::load_from_memory(razer_yellow),
            PerfMode::Balanced(_) => image::load_from_memory(razer_green),
            PerfMode::Custom(_, _, _) => image::load_from_memory(razer_red),
        };

        let (icon_rgba, icon_width, icon_height) = {
            let image = image.expect("Failed to open icon").into_rgba8();
            let (width, height) = image.dimensions();
            let rgba = image.into_raw();
            (rgba, width, height)
        };
        tray_icon::Icon::from_rgba(icon_rgba, icon_width, icon_height).expect("Failed to open icon")
    }
}

fn update(
    tray_icon: &mut tray_icon::TrayIcon,
    new_device_state: DeviceState,
    device: &device::Device,
) -> Result<ProgramState> {
    let new_program_state = ProgramState::new(new_device_state)?;
    tray_icon.set_icon(Some(new_program_state.icon()))?;
    tray_icon.set_tooltip(Some(new_program_state.tooltip()?))?;
    tray_icon.set_menu(Some(Box::new(new_program_state.menu.clone())));
    new_device_state.apply(device)?;

    confy::store(PKG_NAME, None, new_device_state)?;

    log::info!("state updated to {:?}", new_device_state);
    Ok(new_program_state)
}

fn get_logging_file_path() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{}.log", PKG_NAME))
}

fn init_logging_to_file() -> Result<()> {
    use log4rs::append::rolling_file::policy::compound::{
        roll::delete::DeleteRoller, trigger::size::SizeTrigger, CompoundPolicy,
    };
    let policy = CompoundPolicy::new(
        Box::new(SizeTrigger::new(50 << 20)),
        Box::new(DeleteRoller::new()),
    );

    let logfile = log4rs::append::rolling_file::RollingFileAppender::builder()
        .encoder(Box::new(log4rs::encode::pattern::PatternEncoder::new(
            "{h({d(%Y-%m-%d %H:%M:%S)(local)} - {l}: {m}{n})}",
        )))
        .build(get_logging_file_path(), Box::new(policy))?;

    let config = log4rs::config::Config::builder()
        .appender(log4rs::config::Appender::builder().build("logfile", Box::new(logfile)))
        .build(
            log4rs::config::Root::builder()
                .appender("logfile")
                .build(log::LevelFilter::Trace),
        )?;

    log4rs::init_config(config)?;
    Ok(())
}

fn init(tray_icon: &mut tray_icon::TrayIcon, device: &device::Device) -> Result<ProgramState> {
    log::info!(
        "loading config file {}",
        confy::get_configuration_file_path(PKG_NAME, None)?.display()
    );
    let config = confy::load(PKG_NAME, None).unwrap_or_default();
    let state = ProgramState::new(config)?;

    update(tray_icon, state.device_state, device)
}

fn main() -> Result<()> {
    init_logging_to_file()?;
    log::info!("{0} starting {1} {0}", "==".repeat(20), PKG_NAME);

    let device = match device::Device::detect() {
        Ok(d) => {
            log::info!(
                "detected device: {} (0x{:04X})",
                d.info().name,
                d.info().pid
            );
            d
        }
        Err(e) => {
            log::error!("{:?}", e);
            native_dialog::MessageDialog::new()
                .set_type(native_dialog::MessageType::Error)
                .set_text(format!("{:?}", e).as_str())
                .show_alert()?;
            return Err(e);
        }
    };

    let mut tray_icon = TrayIconBuilder::new().build()?;

    let mut state = init(&mut tray_icon, &device)?;

    let menu_channel = MenuEvent::receiver();
    let tray_channel = TrayIconEvent::receiver();
    let event_loop = EventLoopBuilder::new().build();

    let mut last_device_state_check_timestamp = std::time::Instant::now();

    event_loop.run(move |_, _, control_flow| {
        let now = std::time::Instant::now();
        *control_flow = ControlFlow::WaitUntil(now + std::time::Duration::from_millis(1000));

        if let Err(e) = (|| -> Result<()> {
            if let Ok(event) = menu_channel.try_recv() {
                state = update(&mut tray_icon, state.handle_event(event.id.as_ref())?, &device)?;
            }

            if matches!(tray_channel.try_recv(), Ok(event) if event.click_type == tray_icon::ClickType::Left) {
                state = update(&mut tray_icon, state.get_next_perf_mode(), &device)?;
            }

            if now > last_device_state_check_timestamp + std::time::Duration::from_secs(20)
            {
                last_device_state_check_timestamp = now;
                let active_device_state = DeviceState::read(&device)?;
                if active_device_state != state.device_state {
                    log::warn!("overriding externally modified state {:?},",
                              active_device_state);
                    state = update(&mut tray_icon, state.device_state, &device)?;
                }
            }

            Ok(())
        })() {
            log::error!("trying to recover from: {:?}", e);
            match init(&mut tray_icon, &device) {
                Ok(new_state) => state = new_state,
                Err(e) => {
                    log::error!("failed to recover: {:?}", e);
                    *control_flow = ControlFlow::ExitWithCode(1)
                }
            }
        }
    })
}
