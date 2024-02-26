use anyhow::Result;
use strum::IntoEnumIterator;

use librazer::types::{CpuBoost, GpuBoost, LogoMode, MaxFanSpeedMode};
use librazer::{command, device};

use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::{
    menu::{
        AboutMetadata, CheckMenuItem, IsMenuItem, Menu, MenuEvent, PredefinedMenuItem, Submenu,
    },
    TrayIconBuilder, TrayIconEvent,
};

#[derive(Debug, Clone, Copy, PartialEq)]
enum FanSpeed {
    Auto,
    Manual(u16),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PerfMode {
    Silent,
    Balanced(FanSpeed),
    Custom(CpuBoost, GpuBoost, MaxFanSpeedMode),
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct DeviceState {
    perf_mode: PerfMode,
    logo_mode: LogoMode,
}

impl DeviceState {
    fn new(device: &device::Device) -> Result<Self> {
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
                PerfMode::Custom(cpu_boost, gpu_boost, MaxFanSpeedMode::Disable)
            }
        };
        let logo_mode = command::get_logo_mode(device)?;

        Ok(Self {
            perf_mode,
            logo_mode,
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

        match self.logo_mode {
            LogoMode::Static => command::set_logo_mode(device, LogoMode::Static),
            LogoMode::Breathing => command::set_logo_mode(device, LogoMode::Breathing),
            LogoMode::Off => command::set_logo_mode(device, LogoMode::Off),
        }
    }

    fn perf_custom(
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
                    cpu_boost.unwrap_or(CpuBoost::Overclock),
                    gpu_boost.unwrap_or(GpuBoost::High),
                    max_fan_speed.unwrap_or(MaxFanSpeedMode::Disable),
                )
            },
            ..*self
        }
    }
}

struct ProgramState {
    device_state: DeviceState,
    event_handlers: std::collections::HashMap<String, DeviceState>,
}

impl ProgramState {
    fn new(device_state: DeviceState) -> Result<Self> {
        Ok(Self {
            device_state,
            event_handlers: std::collections::HashMap::new(),
        })
    }

    fn menu(&mut self) -> Result<Menu> {
        let menu = Menu::new();
        // header

        // perf
        let perf_modes = Submenu::new("Performance", true);
        // silent
        perf_modes.append(&CheckMenuItem::with_id(
            format!("{:?}", PerfMode::Silent),
            "Silent",
            true,
            self.device_state.perf_mode == PerfMode::Silent,
            None,
        ))?;
        self.event_handlers.insert(
            format!("{:?}", PerfMode::Silent),
            DeviceState {
                perf_mode: PerfMode::Silent,
                ..self.device_state
            },
        );
        // balanced
        let fan_speeds: Vec<CheckMenuItem> = [CheckMenuItem::with_id(
            "fan_speed:auto",
            "Fan: Auto",
            true,
            self.device_state.perf_mode == PerfMode::Balanced(FanSpeed::Auto),
            None,
        )]
        .into_iter()
        .chain((2000..=5000).step_by(500).map(|rpm| {
            let event_id = format!("fan_speed:{}", rpm);
            self.event_handlers.insert(
                event_id.clone(),
                DeviceState {
                    perf_mode: PerfMode::Balanced(FanSpeed::Manual(rpm)),
                    ..self.device_state
                },
            );
            CheckMenuItem::with_id(
                event_id,
                format!("Fan: {} RPM", rpm),
                true,
                self.device_state.perf_mode == PerfMode::Balanced(FanSpeed::Manual(rpm)),
                None,
            )
        }))
        .collect();
        self.event_handlers.insert(
            "fan_speed:auto".to_string(),
            DeviceState {
                perf_mode: PerfMode::Balanced(FanSpeed::Auto),
                ..self.device_state
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
                self.event_handlers.insert(event_id.clone(), {
                    DeviceState {
                        perf_mode: if let PerfMode::Custom(_, gpu_boost, max_fan_speed) =
                            self.device_state.perf_mode
                        {
                            PerfMode::Custom(boost, gpu_boost, max_fan_speed)
                        } else {
                            PerfMode::Custom(boost, GpuBoost::High, MaxFanSpeedMode::Disable)
                        },
                        ..self.device_state
                    }
                });
                let checked =
                    matches!(self.device_state.perf_mode, PerfMode::Custom(b, _, _) if b == boost);
                CheckMenuItem::with_id(event_id, format!("{:?}", boost), !checked, checked, None)
            })
            .collect();
        let gpu_boosts: Vec<CheckMenuItem> = GpuBoost::iter()
            .map(|boost| {
                let event_id = format!("gpu_boost:{:?}", boost);
                self.event_handlers.insert(event_id.clone(), {
                    DeviceState {
                        perf_mode: if let PerfMode::Custom(cpu_boost, _, max_fan_speed) =
                            self.device_state.perf_mode
                        {
                            PerfMode::Custom(cpu_boost, boost, max_fan_speed)
                        } else {
                            PerfMode::Custom(CpuBoost::Boost, boost, MaxFanSpeedMode::Disable)
                        },
                        ..self.device_state
                    }
                });
                let checked =
                    matches!(self.device_state.perf_mode, PerfMode::Custom(_, b, _) if b == boost);
                CheckMenuItem::with_id(event_id, format!("{:?}", boost), !checked, checked, None)
            })
            .collect();

        let max_fan_speed_mode: Vec<CheckMenuItem> = MaxFanSpeedMode::iter()
            .map(|mode| {
                let event_id = format!("max_fan_speed_mode:{:?}", mode);
                self.event_handlers.insert(event_id.clone(), {
                    DeviceState {
                        perf_mode: if let PerfMode::Custom(cpu_boost, gpu_boost, _) =
                            self.device_state.perf_mode
                        {
                            PerfMode::Custom(cpu_boost, gpu_boost, mode)
                        } else {
                            PerfMode::Custom(CpuBoost::Low, GpuBoost::Low, mode)
                        },
                        ..self.device_state
                    }
                });
                CheckMenuItem::with_id(
                    event_id,
                    format!("Max Fan: {:?}", mode),
                    true,
                    matches!(self.device_state.perf_mode, PerfMode::Custom(_, _, m) if m == mode),
                    None,
                )
            })
            .collect();

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
                self.event_handlers.insert(
                    event_id.clone(),
                    DeviceState {
                        logo_mode: mode,
                        ..self.device_state
                    },
                );
                CheckMenuItem::with_id(
                    event_id,
                    format!("{:?}", mode),
                    true,
                    self.device_state.logo_mode == mode,
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

        // footer
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&PredefinedMenuItem::about(
            None,
            Some(AboutMetadata {
                ..Default::default()
            }),
        ))?;
        menu.append(&PredefinedMenuItem::quit(None))?;

        Ok(menu)
    }

    fn handle_event(
        &mut self,
        tray_icon: &mut tray_icon::TrayIcon,
        device: &device::Device,
        event_id: &str,
    ) -> Result<()> {
        let next_device_state = self.event_handlers.get(event_id).ok_or(anyhow::anyhow!(
            "No event handler found for event_id: {}",
            event_id
        ))?;

        if *next_device_state != self.device_state {
            self.device_state = *next_device_state;
            self.event_handlers.clear();
            self.device_state.apply(device)?;
            tray_icon.set_menu(Some(Box::new(self.menu()?)));
            tray_icon.set_tooltip(Some(self.tooltip()?))?;
            tray_icon.set_icon(Some(self.icon()))?; // TODO: handle error
        }

        Ok(())
    }

    fn set_next_perf_mode(
        &mut self,
        tray_icon: &mut tray_icon::TrayIcon,
        device: &device::Device,
    ) -> Result<()> {
        match self.device_state.perf_mode {
            PerfMode::Silent => self.handle_event(tray_icon, device, "fan_speed:auto"),
            PerfMode::Balanced(..) => self.handle_event(tray_icon, device, "cpu_boost:Overclock"),
            PerfMode::Custom(..) => self.handle_event(
                tray_icon,
                device,
                format!("{:?}", PerfMode::Silent).as_ref(),
            ),
        }
    }

    fn tooltip(&self) -> Result<String> {
        use std::fmt::Write;
        let mut info = String::new();

        match self.device_state.perf_mode {
            PerfMode::Silent => writeln!(&mut info, "Perf: Silent")?,
            PerfMode::Balanced(FanSpeed::Auto) => {
                writeln!(&mut info, "Perf: Balanced")?;
                writeln!(&mut info, "Fan: Auto")?
            }
            PerfMode::Balanced(FanSpeed::Manual(rpm)) => {
                writeln!(&mut info, "Perf: Balanced")?;
                writeln!(&mut info, "Fan: {} RPM", rpm)?
            }
            PerfMode::Custom(cpu_boost, gpu_boost, max_fan_speed) => {
                writeln!(&mut info, "Perf: Custom")?;
                writeln!(&mut info, "CPU: {:?}", cpu_boost)?;
                writeln!(&mut info, "GPU: {:?}", gpu_boost)?;
                writeln!(&mut info, "Max Fan: {:?}", max_fan_speed)?;
            }
        }

        writeln!(&mut info, "Logo: {:?}", self.device_state.logo_mode)?;

        Ok(info.trim().to_string())
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

fn main() -> Result<()> {
    const RAZER_BLADE_16_2023_PID: u16 = 0x029f;
    let device = device::Device::new(RAZER_BLADE_16_2023_PID)?;

    let mut state = ProgramState::new(DeviceState::new(&device)?)?;

    let mut tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(state.menu()?))
        .with_tooltip(state.tooltip()?)
        .with_icon(state.icon())
        .build()?;

    let menu_channel = MenuEvent::receiver();
    let tray_channel = TrayIconEvent::receiver();

    let event_loop = EventLoopBuilder::new().build();

    event_loop.run(move |_event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(
            std::time::Instant::now() + std::time::Duration::from_millis(500),
        );

        if let Ok(event) = menu_channel.try_recv() {
            println!("{event:?}");
            if let Err(e) = state.handle_event(&mut tray_icon, &device, event.id.as_ref()) {
                eprintln!("Failed to handle event: {:?} error: {:?}", event, e);
                *control_flow = ControlFlow::ExitWithCode(1);
            }

            return;
        }

        if let Ok(event) = tray_channel.try_recv() {
            println!("{event:?}");
            if event.click_type == tray_icon::ClickType::Left {
                if let Err(e) = state.set_next_perf_mode(&mut tray_icon, &device) {
                    eprintln!("Failed to set next perf mode: {:?}", e);
                    *control_flow = ControlFlow::ExitWithCode(1);
                }
            }
        }
    })
}
