use anyhow::{ensure, Result};
use strum::IntoEnumIterator;

use librazer::types::{CpuBoost, GpuBoost, LogoMode, MaxFanSpeedMode};
use librazer::{command, device};

use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::{
    menu::{
        AboutMetadata, CheckMenuItem, IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem,
        Submenu,
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
    Custom(CpuBoost, GpuBoost, Option<MaxFanSpeedMode>),
}

#[derive(Debug, Clone, Copy)]
struct DeviceState {
    perf_mode: PerfMode,
    logo_mode: LogoMode,
}

impl DeviceState {
    fn new(device: &device::Device) -> Result<Self> {
        let perf_mode = match command::get_perf_mode(&device)? {
            (librazer::types::PerfMode::Silent, _) => PerfMode::Silent,
            (librazer::types::PerfMode::Balanced, librazer::types::FanMode::Auto) => {
                PerfMode::Balanced(FanSpeed::Auto)
            }
            (librazer::types::PerfMode::Balanced, librazer::types::FanMode::Manual) => {
                let fan_speed = command::get_fan_rpm(&device, librazer::types::FanZone::Zone1)?;
                PerfMode::Balanced(FanSpeed::Manual(fan_speed))
            }
            (librazer::types::PerfMode::Custom, _) => {
                let cpu_boost = command::get_cpu_boost(&device)?;
                let gpu_boost = command::get_gpu_boost(&device)?;
                PerfMode::Custom(cpu_boost, gpu_boost, None)
            }
        };
        let logo_mode = command::get_logo_mode(&device)?;

        Ok(Self {
            perf_mode,
            logo_mode,
        })
    }
}

struct ProgramState {
    device_state: DeviceState,
    event_handlers: std::collections::HashMap<
        String,
        Box<dyn Fn(&Self, &device::Device, &str) -> Result<ProgramState>>,
    >,
}

impl ProgramState {
    fn new(device_state: DeviceState) -> Result<Self> {
        Ok(Self {
            device_state,
            event_handlers: std::collections::HashMap::new(),
        })
    }

    fn menu(&mut self) -> Result<Menu> {
        /*
        let fan_speeds: Vec<MenuItem> = [MenuItem::with_id("fan_speed:auto", "Auto", true, None)]
            .into_iter()
            .chain((2000..=5000).step_by(500).map(|rpm| {
                MenuItem::with_id(
                    format!("fan_speed:manual:{}", rpm),
                    format!("{}", rpm),
                    true,
                    None,
                )
            }))
            .collect();

        let menu = Menu::with_items(&[&Submenu::with_id_and_items(
            "perf",
            "Performance Modes",
            true,
            &[
                &CheckMenuItem::with_id("perf_mode:silent", "Silent", true, false, None),
                &Submenu::with_id_and_items(
                    "perf_mode:balanced",
                    "Balanced",
                    true,
                    &[&Submenu::with_items(
                        "Fan Speed",
                        true,
                        &fan_speeds
                            .iter()
                            .map(|i| i as &dyn IsMenuItem)
                            .collect::<Vec<_>>(),
                    )?],
                )?,
                &Submenu::with_id_and_items("perf_mode:custom", "Custom", true, &[])?,
            ],
        )?])?;
        */

        macro_rules! add_handler {
            ($key:expr, $body:expr) => {{
                self.event_handlers.insert(
                    $key.to_string(),
                    Box::new(
                        move |state: &Self,
                              dev: &device::Device,
                              event_id: &str|
                              -> Result<ProgramState> {
                            let mut new_state = ProgramState::new(state.device_state).unwrap();
                            ($body(&mut new_state, dev, event_id))?;
                            Ok(new_state)
                        },
                    ),
                );
            }};
        }

        let menu = Menu::new();
        // header

        // perf
        let perf_modes = Submenu::new("Performance", true);
        // silent
        perf_modes.append(&MenuItem::with_id("perf_mode:silent", "Silent", true, None))?;
        add_handler!("perf_mode:silent", {
            |new_state: &mut Self, device: &device::Device, event_id: &str| -> Result<()> {
                new_state.device_state.perf_mode = PerfMode::Silent;
                command::set_perf_mode(device, librazer::types::PerfMode::Silent)
            }
        });
        // balanced
        let fan_speeds: Vec<MenuItem> =
            [MenuItem::with_id("fan_speed:auto", "Fan: Auto", true, None)]
                .into_iter()
                .chain((2000..=5000).step_by(500).map(|rpm| {
                    let event_id = format!("fan_speed:{}", rpm);
                    add_handler!(event_id, {
                        |new_state: &mut Self, device: &device::Device, _: &str| -> Result<()> {
                            new_state.device_state.perf_mode =
                                PerfMode::Balanced(FanSpeed::Manual(rpm));
                            command::set_perf_mode(device, librazer::types::PerfMode::Balanced)?;
                            command::set_fan_mode(device, librazer::types::FanMode::Manual)?;
                            command::set_fan_rpm(device, rpm)
                        }
                    });
                    MenuItem::with_id(event_id, format!("Fan: {} RPM", rpm), true, None)
                }))
                .collect();
        add_handler!("fan_speed:auto", {
            |new_state: &mut Self, device: &device::Device, _: &str| -> Result<()> {
                new_state.device_state.perf_mode = PerfMode::Balanced(FanSpeed::Auto);
                command::set_perf_mode(device, librazer::types::PerfMode::Balanced)
            }
        });

        perf_modes.append(&Submenu::with_items(
            "Balanced",
            true,
            &fan_speeds
                .iter()
                .map(|i| i as &dyn IsMenuItem)
                .collect::<Vec<_>>(),
        )?)?;

        menu.append(&perf_modes)?;

        // logo
        menu.append(&PredefinedMenuItem::separator())?;
        let modes = LogoMode::iter()
            .map(|mode| {
                let event_id = format!("logo_mode:{:?}", mode);
                add_handler!(event_id, {
                    |new_state: &mut Self, device, _| -> Result<()> {
                        new_state.device_state.logo_mode = mode;
                        command::set_logo_mode(device, mode)
                    }
                });
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

    fn handle_event(&self, device: &device::Device, event: &MenuEvent) -> Result<ProgramState> {
        match self.event_handlers.get(event.id.as_ref()) {
            Some(handler) => handler(self, device, event.id.as_ref()),
            None => {
                anyhow::bail!("Missing handler for event: {:?}", event)
            }
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
                if let Some(max_fan_speed) = max_fan_speed {
                    writeln!(&mut info, "Max Fan Speed: {:?}", max_fan_speed)?;
                }
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
        *control_flow = ControlFlow::Wait;

        if let Ok(event) = menu_channel.try_recv() {
            match state.handle_event(&device, &event) {
                Ok(new_state) => {
                    state = new_state;
                    tray_icon.set_menu(Some(Box::new(state.menu().unwrap())));
                    let _ = tray_icon.set_tooltip(Some(state.tooltip().unwrap())); // TODO: handle error
                    let _ = tray_icon.set_icon(Some(state.icon())); // TODO: handle error
                }
                Err(e) => {
                    eprintln!("Failed to get new state for event: {:?}", e);
                    *control_flow = ControlFlow::ExitWithCode(1);
                }
            }

            println!("{event:?}");
        }

        if let Ok(event) = tray_channel.try_recv() {
            println!("{event:?}");
        }
    })
}
