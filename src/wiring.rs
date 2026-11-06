// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later
//! The one wiring module (ADR-078 item 3, D-13): builds Core's engine,
//! starts linux-hw's hardware shell and hands both to the display shell.
//! Matching on a `Command` or an `Event` is display or hardware work and
//! belongs in a shell, never here; `lint:no-behaviour` fails the build
//! when either appears outside this file.
#[cfg(feature = "gtk")]
use gtk_display::core_ui::engine_link::EngineHandle;
#[cfg(feature = "gtk")]
use gtk_display::core_ui::hardware_link::{HardwareLink, StartedHardware};
#[cfg(feature = "gtk")]
use gtk4::gio;
#[cfg(feature = "gtk")]
use gtk4::prelude::*;
#[cfg(feature = "gtk")]
use std::{cell::RefCell, rc::Rc};
use vauchi_app::ui::AppEngine;
use vauchi_contract::{Command, CommandTarget, Event};
use vauchi_core::settings::NetworkCost;
#[cfg(feature = "gtk")]
use vauchi_linux_hw::{Backends, HostDevices};
use vauchi_linux_hw::{ExecuteError, LinuxHardware};

use crate::engine::CommandEngine;
use crate::engine::CoreEngine;

/// App-side command routing and observation forwarding, independent of a display.
pub struct HardwareBridge<E = CoreEngine> {
    engine: E,
    hardware: LinuxHardware,
    commands: Vec<Command>,
}

impl<E: CommandEngine> HardwareBridge<E> {
    pub fn new(engine: E, hardware: LinuxHardware) -> Result<Self, ExecuteError> {
        let mut bridge = Self {
            engine,
            hardware,
            commands: Vec::new(),
        };
        let capabilities = bridge.hardware.report();
        bridge.dispatch(capabilities)?;
        // Core's subscription starts invalidated (#1029). Consume that token
        // before the initial snapshot so the first poll preserves live input.
        let _ = bridge.engine.take_invalidation();
        bridge.route(bridge.engine.initial_commands())?;
        Ok(bridge)
    }

    pub fn dispatch(&mut self, event: Event) -> Result<(), ExecuteError> {
        self.route(self.engine.dispatch(event))
    }

    pub fn take_commands(&mut self) -> Vec<Command> {
        std::mem::take(&mut self.commands)
    }

    /// Process a pending repaint and one ready hardware observation.
    pub fn poll(&mut self) -> Result<bool, ExecuteError> {
        let refreshed = self.refresh_presentation()?;
        match self.hardware.pop() {
            Some(event) => {
                self.dispatch(event)?;
                Ok(true)
            }
            None => Ok(refreshed),
        }
    }

    /// Wait for an observation while the host has no input to forward.
    /// Repaints are checked before waiting and after a timeout; the host
    /// chooses a bounded timeout to keep background updates responsive.
    pub fn wait_for_event(&mut self, timeout: std::time::Duration) -> Result<bool, ExecuteError> {
        if self.poll()? {
            return Ok(true);
        }
        match self.hardware.wait_for_event(timeout) {
            Some(event) => {
                self.dispatch(event)?;
                Ok(true)
            }
            None => self.refresh_presentation(),
        }
    }

    fn refresh_presentation(&mut self) -> Result<bool, ExecuteError> {
        if !self.engine.take_invalidation() {
            return Ok(false);
        }
        self.route(self.engine.initial_commands())?;
        Ok(true)
    }

    fn route(&mut self, commands: Vec<Command>) -> Result<(), ExecuteError> {
        let mut failure = None;
        for command in commands {
            if command.target() == CommandTarget::Hardware {
                match self.hardware.execute(command) {
                    Ok(()) | Err(ExecuteError::Unsupported) => {}
                    Err(error) => {
                        failure.get_or_insert(error);
                    }
                }
            } else {
                self.commands.push(command);
            }
        }
        failure.map_or(Ok(()), Err)
    }
}

/// Shared storage, optional test identity, locale report and Core bootstrap.
pub fn start_shared_engine(
    reset_for_testing: bool,
) -> Result<CoreEngine, Box<dyn std::error::Error>> {
    let mut vauchi = crate::storage::init_vauchi()?;
    maybe_seed_test_identity(&mut vauchi, reset_for_testing);
    let mut app_engine = AppEngine::new(vauchi);
    crate::locale::report_system_locale(&mut app_engine, |name| std::env::var(name).ok());
    // CDN languages, themes and social networks: Core runs the update cycle
    // and announces applied content as a Vauchi event, which the shell
    // re-renders on (vauchi/private#733 D10).
    if let Some(data_dir) = app_engine
        .vauchi()
        .config()
        .storage_path
        .parent()
        .map(std::path::Path::to_path_buf)
    {
        app_engine.start_content_updates(&data_dir, vauchi_app::ui::ContentRunOptions::default());
    }
    app_engine.bootstrap();
    Ok(CoreEngine::new(app_engine))
}

#[cfg(feature = "gtk")]
pub fn start_engine() -> EngineHandle {
    let engine: EngineHandle = Rc::new(
        start_shared_engine(gtk_display::shell::reset_for_testing_requested())
            .expect("Failed to initialize Vauchi backend"),
    );
    watch_network_cost(&engine);
    engine
}

/// Creates a test identity so the app skips onboarding and lands on home.
///
/// Triggered by the `--reset-for-testing` flag (the shell strips it before
/// GTK's parser) or `VAUCHI_TEST_SEED=1`, which CI's AT-SPI harness sets.
fn maybe_seed_test_identity(vauchi: &mut vauchi_core::api::Vauchi, cli_flag: bool) {
    let env_flag = std::env::var("VAUCHI_TEST_SEED").as_deref() == Ok("1");
    if !cli_flag && !env_flag {
        return;
    }
    if vauchi.has_identity() {
        eprintln!("[vauchi] --reset-for-testing: identity already exists, skipping");
        return;
    }
    match vauchi.create_identity("Test User") {
        Ok(()) => eprintln!("[vauchi] --reset-for-testing: test identity created"),
        Err(e) => eprintln!("[vauchi] --reset-for-testing: failed to create identity: {e}"),
    }
}

/// GIO monitors that read metering from NetworkManager, directly or through
/// the Flatpak portal. The netlink fallback reports every network as
/// unmetered, so its answer is unknown instead (vauchi/private#733 D6).
const MONITORS_THAT_KNOW_METERING: [&str; 2] = ["GNetworkMonitorNM", "GNetworkMonitorPortal"];

pub fn network_cost(monitor_type: &str, available: bool, metered: bool) -> NetworkCost {
    if !available || !MONITORS_THAT_KNOW_METERING.contains(&monitor_type) {
        NetworkCost::Unknown
    } else if metered {
        NetworkCost::Metered
    } else {
        NetworkCost::Unmetered
    }
}

/// Reports what the network costs now and whenever it changes, so Core can
/// hold "Wi-Fi only" content updates back on a metered one. Core decides.
#[cfg(feature = "gtk")]
fn watch_network_cost(engine: &EngineHandle) {
    let monitor = gio::NetworkMonitor::default();
    report_network_cost(engine, &monitor);
    let changed = engine.clone();
    monitor.connect_network_changed(move |monitor, _| report_network_cost(&changed, monitor));
    let metered = engine.clone();
    monitor.connect_network_metered_notify(move |monitor| report_network_cost(&metered, monitor));
}

#[cfg(feature = "gtk")]
fn report_network_cost(engine: &EngineHandle, monitor: &gio::NetworkMonitor) {
    let cost = network_cost(
        monitor.type_().name(),
        monitor.is_network_available(),
        monitor.is_network_metered(),
    );
    // A platform fact: Core answers with no commands.
    let _ = engine.dispatch(Event::NetworkCostChanged { cost });
}

/// The host's hardware: probe the devices, attach a port per device.
#[cfg(feature = "gtk")]
pub fn start_hardware() -> Option<StartedHardware> {
    let devices = HostDevices::probe();
    start_hardware_with(Backends::host(&devices), devices)
}

/// The hardware shell over the given backends; `None` when its runtime
/// cannot start, and the display shell then keeps every command.
#[cfg(feature = "gtk")]
pub fn start_hardware_with(backends: Backends, devices: HostDevices) -> Option<StartedHardware> {
    let hardware = match LinuxHardware::with_backends(backends, devices) {
        Ok(hardware) => hardware,
        Err(error) => {
            eprintln!("[linux-app] linux-hw did not start: {error}");
            return None;
        }
    };
    let link = LinuxHardwareLink::new(hardware);
    let capabilities = Some(link.report());
    Some(StartedHardware {
        link: Rc::new(link),
        capabilities,
    })
}

/// The start line names the Core this binary links, so a log shows which
/// Core a report came from (ADR-078 item 7).
pub fn core_version() -> &'static str {
    vauchi_core::VERSION
}

#[cfg(feature = "tui")]
pub fn run_tui() -> Result<(), Box<dyn std::error::Error>> {
    use crate::terminal_tui::{CrosstermEvents, TerminalMode, TuiTerminal};
    let engine = start_shared_engine(false)?;
    let hardware = LinuxHardware::start()?;
    let bridge = HardwareBridge::new(engine, hardware)?;
    let mode = TerminalMode::enter()?;
    let backend = ratatui_crossterm::CrosstermBackend::new(std::io::stdout());
    let mut output = ratatui_core::terminal::Terminal::new(backend)?;
    let mut terminal =
        TuiTerminal::new(&mut output, CrosstermEvents, terminal_feedback).with_mode(mode);
    crate::terminal::run_terminal(bridge, &mut terminal)?;
    Ok(())
}

#[cfg(feature = "tui")]
fn terminal_feedback(
    commands: &[Command],
    feedback: &mut crate::terminal_tui::TerminalFeedback,
) -> std::io::Result<()> {
    for command in commands {
        match command {
            Command::ShowToast { toast } => feedback.status = Some(toast.message.clone()),
            Command::PresentAlert { alert } => {
                feedback.alert = Some((alert.title.clone(), alert.message.clone()));
            }
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "terminal effect has no native adapter",
                ));
            }
        }
    }
    Ok(())
}

#[cfg(feature = "gtk")]
struct LinuxHardwareLink(RefCell<LinuxHardware>);

#[cfg(feature = "gtk")]
impl LinuxHardwareLink {
    fn new(hardware: LinuxHardware) -> Self {
        Self(RefCell::new(hardware))
    }

    /// The capabilities to report to Core once, at start.
    fn report(&self) -> Event {
        self.0.borrow().report()
    }
}

/// The commands linux-hw has a port for (`HardwareShell::execute`).
#[cfg(feature = "gtk")]
fn ported(command: &Command) -> bool {
    matches!(
        command,
        Command::BleStartScanning { .. }
            | Command::BleStopScanning
            | Command::BleStartAdvertising { .. }
            | Command::BleStopAdvertising
            | Command::BleConnect { .. }
            | Command::BleWriteCharacteristic { .. }
            | Command::BleReadCharacteristic { .. }
            | Command::BleDisconnect { .. }
            | Command::NfcActivate { .. }
            | Command::NfcSendApdu { .. }
            | Command::NfcDeactivate
            | Command::AudioEmitChallenge { .. }
            | Command::AudioListenForResponse { .. }
            | Command::AudioStop
            | Command::CameraCapture { .. }
            | Command::CameraStop { .. }
            | Command::DirectSend { .. }
    )
}

#[cfg(feature = "gtk")]
impl HardwareLink for LinuxHardwareLink {
    fn execute(&self, command: &Command) -> bool {
        if !ported(command) {
            return false;
        }
        // A port without a device answers Unsupported: report nothing.
        let _ = self.0.borrow_mut().execute(command.clone());
        true
    }

    fn pop(&self) -> Option<Event> {
        self.0.borrow_mut().pop()
    }
}
