use anyhow::Result;
use serde::{Deserialize, Serialize};
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
            logo_mode: LogoMode::Off,
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
            self.device_state.perf_mode != PerfMode::Silent,
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
            self.device_state.perf_mode != PerfMode::Balanced(FanSpeed::Auto),
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
                self.device_state.perf_mode != PerfMode::Balanced(FanSpeed::Manual(rpm)),
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
                self.event_handlers
                    .insert(event_id.clone(), self.device_state.delta(boost));
                let checked =
                    matches!(self.device_state.perf_mode, PerfMode::Custom(b, _, _) if b == boost);
                CheckMenuItem::with_id(event_id, format!("{:?}", boost), !checked, checked, None)
            })
            .collect();

        let gpu_boosts: Vec<CheckMenuItem> = GpuBoost::iter()
            .map(|boost| {
                let event_id = format!("gpu_boost:{:?}", boost);
                self.event_handlers
                    .insert(event_id.clone(), self.device_state.delta(boost));
                let checked =
                    matches!(self.device_state.perf_mode, PerfMode::Custom(_, b, _) if b == boost);
                CheckMenuItem::with_id(event_id, format!("{:?}", boost), !checked, checked, None)
            })
            .collect();

        let max_fan_speed_mode: Vec<CheckMenuItem> = MaxFanSpeedMode::iter()
            .map(|mode| {
                let event_id = format!("max_fan_speed_mode:{:?}", mode);
                self.event_handlers
                    .insert(event_id.clone(), self.device_state.delta(mode));
                let checked =
                    matches!(self.device_state.perf_mode, PerfMode::Custom(_, _, m) if m == mode);
                CheckMenuItem::with_id(
                    event_id,
                    format!("Max Fan: {:?}", mode),
                    !checked,
                    checked,
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
                    self.device_state.logo_mode != mode,
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
            self.update_state(tray_icon, device, *next_device_state)?;
        }

        Ok(())
    }

    fn update_state(
        &mut self,
        tray_icon: &mut tray_icon::TrayIcon,
        device: &device::Device,
        next_device_state: DeviceState,
    ) -> Result<()> {
        self.device_state = next_device_state;
        self.event_handlers.clear();
        self.device_state.apply(device)?;
        tray_icon.set_menu(Some(Box::new(self.menu()?)));
        tray_icon.set_tooltip(Some(self.tooltip()?))?;
        tray_icon.set_icon(Some(self.icon()))?;
        Ok(confy::store("razer-tray", None, self.device_state)?)
    }

    fn set_next_perf_mode(
        &mut self,
        tray_icon: &mut tray_icon::TrayIcon,
        device: &device::Device,
    ) -> Result<()> {
        self.update_state(
            tray_icon,
            device,
            DeviceState {
                perf_mode: match self.device_state.perf_mode {
                    PerfMode::Silent => PerfMode::Balanced(FanSpeed::Auto),
                    PerfMode::Balanced(..) => {
                        PerfMode::Custom(CpuBoost::Boost, GpuBoost::High, MaxFanSpeedMode::Disable)
                    }
                    PerfMode::Custom(..) => PerfMode::Silent,
                },
                ..self.device_state
            },
        )
    }

    fn synchronize(
        &mut self,
        tray_icon: &mut tray_icon::TrayIcon,
        device: &device::Device,
    ) -> Result<()> {
        let next_device_state = DeviceState::new(device)?;
        if next_device_state != self.device_state {
            eprintln!("Device state changed externally {:?}", next_device_state);
            self.update_state(tray_icon, device, self.device_state)?;
        }
        Ok(())
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

    let mut state = ProgramState::new(confy::load("razer-tray", None)?)?;

    let mut tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(state.menu()?))
        .with_tooltip(state.tooltip()?)
        .with_icon(state.icon())
        .build()?;

    let menu_channel = MenuEvent::receiver();
    let tray_channel = TrayIconEvent::receiver();

    let event_loop = EventLoopBuilder::new().build();

    let mut idle_counter = 0;

    event_loop.run(move |_event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(
            std::time::Instant::now() + std::time::Duration::from_millis(1000),
        );

        if let Err(e) = (|| -> Result<()> {
            if let Ok(event) = menu_channel.try_recv() {
                state.handle_event(&mut tray_icon, &device, event.id.as_ref())?;
            }
            if let Ok(event) = tray_channel.try_recv() {
                if event.click_type == tray_icon::ClickType::Left {
                    state.set_next_perf_mode(&mut tray_icon, &device)?;
                }
            }
            if matches!(
                _event,
                tao::event::Event::NewEvents(tao::event::StartCause::ResumeTimeReached { .. })
            ) {
                idle_counter += 1;
                if idle_counter % 10 == 0 {
                    idle_counter = 0;
                    state.synchronize(&mut tray_icon, &device)?;
                }
            }

            Ok(())
        })() {
            eprintln!("Failed with error: {:?}", e);
            *control_flow = ControlFlow::ExitWithCode(1);
        }
    })
}
