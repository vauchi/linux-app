// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later
//! The one wiring module (ADR-078 item 3, D-13): builds Core's engine,
//! starts linux-hw's hardware shell and hands both to the display shell.
//! Matching on a `Command` or an `Event` is display or hardware work and
//! belongs in a shell, never here; `lint:no-behaviour` fails the build
//! when either appears outside this file.
#[cfg(feature = "gtk")]
use gtk_display::core_ui::engine_link::{EngineHandle, EngineLink};
#[cfg(feature = "gtk")]
use gtk4::gio;
#[cfg(feature = "gtk")]
use gtk4::glib;
#[cfg(feature = "gtk")]
use gtk4::prelude::*;
#[cfg(feature = "gtk")]
use std::{cell::RefCell, rc::Rc};
use vauchi_app::ui::AppEngine;
use vauchi_contract::{Command, CommandTarget, Event};
use vauchi_core::settings::NetworkCost;
#[cfg(feature = "gtk")]
use vauchi_linux_hw::StartError;
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

    #[cfg(feature = "camera-pipewire")]
    pub fn preview_directory(&self) -> Option<vauchi_linux_hw::preview_source::PreviewDirectory> {
        self.hardware.preview_directory()
    }

    /// The next hardware observation, if any. A host with its own main
    /// loop pumps it (the GTK pairing's 50 ms timer); `poll` and
    /// `wait_for_event` cover hosts that block instead.
    pub fn pop_event(&mut self) -> Option<Event> {
        self.hardware.pop()
    }

    /// Whether Core invalidated the presentation since the last call.
    pub fn invalidated(&self) -> bool {
        self.engine.take_invalidation()
    }

    /// Core's current command projection with the hardware commands
    /// routed away: only the display batch the renderer applies.
    pub fn current_commands(&mut self) -> Result<Vec<Command>, ExecuteError> {
        self.route(self.engine.initial_commands())?;
        Ok(self.take_commands())
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
        let mut unavailable = Vec::new();
        for command in commands {
            if command.target() == CommandTarget::Hardware {
                let transport = unavailable_transport(&command);
                match self.hardware.execute(command) {
                    Ok(()) => {}
                    Err(ExecuteError::Unsupported) => {
                        // No Linux port serves this command. Tell Core the
                        // transport is unavailable so it clears the pending
                        // request instead of waiting out the timeout; the
                        // renderer's hardware fallback did this before
                        // (#1252).
                        if let Some(transport) = transport
                            && !unavailable.contains(&transport)
                        {
                            unavailable.push(transport);
                        }
                    }
                    Err(error) => {
                        failure.get_or_insert(error);
                    }
                }
            } else {
                self.commands.push(command);
            }
        }
        for transport in unavailable {
            self.dispatch(Event::HardwareUnavailable {
                transport: transport.to_string(),
            })?;
        }
        failure.map_or(Ok(()), Err)
    }
}

/// The bridge is the host tui-display's shared loop drives (private#1234).
#[cfg(feature = "tui")]
impl<E: CommandEngine> tui_display::host::TerminalHost for HardwareBridge<E> {
    type Error = ExecuteError;

    fn dispatch(&mut self, event: Event) -> Result<(), ExecuteError> {
        HardwareBridge::dispatch(self, event)
    }

    fn poll(&mut self) -> Result<bool, ExecuteError> {
        HardwareBridge::poll(self)
    }

    fn take_commands(&mut self) -> Vec<Command> {
        HardwareBridge::take_commands(self)
    }
}

/// The transport name Core learns when no Linux port serves the command.
/// Names match the ones the GTK renderer's fallback reported (#1252):
/// "Camera" (capital C) is the name Core answers a missing QR scanner
/// with a toast rather than a failed exchange.
fn unavailable_transport(command: &Command) -> Option<&'static str> {
    match command {
        Command::AudioEmitChallenge { .. } | Command::AudioListenForResponse { .. } => {
            Some("Audio")
        }
        Command::BleStartScanning { .. }
        | Command::BleStartAdvertising { .. }
        | Command::BleConnect { .. }
        | Command::BleWriteCharacteristic { .. }
        | Command::BleReadCharacteristic { .. } => Some("BLE"),
        Command::NfcActivate { .. } | Command::NfcSendApdu { .. } => Some("NFC"),
        Command::QrRequestScan => Some("Camera"),
        Command::LocationRequest { .. } => Some("location"),
        Command::ImageCaptureFromCamera => Some("camera"),
        Command::SwitchCamera { .. } => Some("camera_switch"),
        _ => None,
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

/// The GTK display's engine: the app-owned bridge (#1252). Hardware
/// commands are routed to linux-hw before the renderer can see them, and
/// hardware observations are pumped into Core on the GTK main loop.
#[cfg(feature = "gtk")]
pub struct BridgeEngine<E = CoreEngine> {
    bridge: RefCell<HardwareBridge<E>>,
    // The bridge consumed Core's initial invalidation token for its own
    // snapshot; the shell still needs its boot repaint (#1252).
    boot_repaint: std::cell::Cell<bool>,
}

#[cfg(feature = "gtk")]
impl<E: CommandEngine + 'static> BridgeEngine<E> {
    /// Build the display bridge from the native startup result.
    pub fn from_hardware_start(
        engine: E,
        hardware: Result<LinuxHardware, StartError>,
    ) -> Result<Rc<Self>, ExecuteError> {
        Self::new(engine, hardware_or_degraded(hardware))
    }

    pub fn new(engine: E, hardware: LinuxHardware) -> Result<Rc<Self>, ExecuteError> {
        #[cfg(feature = "camera-pipewire")]
        crate::camera_preview::install_gtk(hardware.preview_directory());
        let mut bridge = HardwareBridge::new(engine, hardware)?;
        // The constructor routed Core's boot projection once for its
        // hardware side effects. initial_commands() recomputes that
        // projection on every repaint, so drop the queued copy instead
        // of returning it twice.
        let _ = bridge.take_commands();
        Ok(Rc::new(Self {
            bridge: RefCell::new(bridge),
            boot_repaint: std::cell::Cell::new(true),
        }))
    }

    /// Pump hardware observations every 50 ms on the main loop, as the
    /// deleted renderer-side link did. The display's invalidation poll
    /// applies the display commands that come back; the timer stops once
    /// the last handle is dropped. Call from the GTK main thread, once the
    /// default main context exists.
    pub fn install_hardware_pump(this: &Rc<Self>) {
        let weak = Rc::downgrade(this);
        glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
            let Some(this) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let mut bridge = this.bridge.borrow_mut();
            while let Some(event) = bridge.pop_event() {
                if let Err(error) = bridge.dispatch(event) {
                    eprintln!("[linux-app] hardware event delivery failed: {error}");
                }
            }
            glib::ControlFlow::Continue
        });
    }
}

#[cfg(feature = "gtk")]
impl<E: CommandEngine> EngineLink for BridgeEngine<E> {
    fn dispatch(&self, event: Event) -> Vec<Command> {
        let mut bridge = self.bridge.borrow_mut();
        if let Err(error) = bridge.dispatch(event) {
            eprintln!("[linux-app] hardware command delivery failed: {error}");
        }
        bridge.take_commands()
    }

    fn initial_commands(&self) -> Vec<Command> {
        let result = self.bridge.borrow_mut().current_commands();
        match result {
            Ok(commands) => commands,
            Err(error) => {
                eprintln!("[linux-app] hardware command delivery failed: {error}");
                self.bridge.borrow_mut().take_commands()
            }
        }
    }

    fn take_invalidation(&self) -> bool {
        self.boot_repaint.replace(false) || self.bridge.borrow().invalidated()
    }
}

/// The host's hardware runtime, or an empty shell when the runtime cannot
/// start: the app must still boot (#1252). A degraded shell reports no
/// devices, so Core sees the real capabilities instead of the process
/// dying before the first window.
#[cfg(feature = "gtk")]
fn hardware_or_degraded(started: Result<LinuxHardware, StartError>) -> LinuxHardware {
    match started {
        Ok(hardware) => hardware,
        Err(error) => {
            eprintln!(
                "[linux-app] linux-hw runtime did not start ({error}); \
                 starting with no hardware devices"
            );
            LinuxHardware::without_devices()
        }
    }
}

#[cfg(feature = "gtk")]
pub fn start_engine() -> EngineHandle {
    let engine = start_shared_engine(gtk_display::shell::reset_for_testing_requested())
        .expect("Failed to initialize Vauchi backend");
    let bridge = BridgeEngine::from_hardware_start(engine, LinuxHardware::start())
        .expect("Failed to start the hardware bridge");
    BridgeEngine::install_hardware_pump(&bridge);
    let engine: EngineHandle = bridge;
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

/// The start line names the Core this binary links, so a log shows which
/// Core a report came from (ADR-078 item 7).
pub fn core_version() -> &'static str {
    vauchi_core::VERSION
}

/// Removes every `--reset-for-testing` and reports whether there was one, so
/// the toolkit's argument parser never sees the flag.
pub fn take_reset_for_testing(args: Vec<String>) -> (Vec<String>, bool) {
    let before = args.len();
    let kept: Vec<String> = args
        .into_iter()
        .filter(|arg| arg != "--reset-for-testing")
        .collect();
    let requested = kept.len() != before;
    (kept, requested)
}

/// Offers this launch's arguments to a running app before starting one:
/// when `hand_off` reports the links taken, nothing starts and the launch
/// exits cleanly, so one user's data is never opened twice (private#650).
pub fn launch_unless_handed_off<E>(
    args: &[String],
    hand_off: impl FnOnce(&[String]) -> bool,
    launch: impl FnOnce(&[String]) -> Result<i32, E>,
) -> Result<i32, E> {
    if hand_off(args) {
        return Ok(0);
    }
    launch(args)
}

#[cfg(feature = "qt")]
pub fn run_qt() -> Result<i32, Box<dyn std::error::Error>> {
    let (args, reset_for_testing) = take_reset_for_testing(std::env::args().collect());
    launch_unless_handed_off(&args, qt_display::hand_off_links, |args| {
        let engine = start_shared_engine(reset_for_testing)?;
        let hardware = LinuxHardware::start()?;
        let bridge = HardwareBridge::new(engine, hardware)?;
        Ok(crate::qt::run(args, bridge)?)
    })
}

#[cfg(feature = "tui")]
pub fn run_tui() -> Result<(), Box<dyn std::error::Error>> {
    use tui_display::terminal::{CrosstermEvents, TerminalMode, TuiTerminal};
    let engine = start_shared_engine(false)?;
    let hardware = LinuxHardware::start()?;
    let bridge = HardwareBridge::new(engine, hardware)?;
    let shutdown = crate::terminal_signals::SignalShutdown::new()?;
    let mode = TerminalMode::enter()?;
    let backend = ratatui_crossterm::CrosstermBackend::new(std::io::stdout());
    let mut output = ratatui_core::terminal::Terminal::new(backend)?;
    let mut terminal = TuiTerminal::new(&mut output, CrosstermEvents, tui_feedback).with_mode(mode);
    tui_display::host::run_terminal_with_shutdown(bridge, &mut terminal, || shutdown.requested())?;
    Ok(())
}

/// The full-screen terminal's display effects. One it has no form for is
/// left out, as the old `tui` binary did: ending the app on it took the
/// device link down (vauchi/private#1233).
#[cfg(feature = "tui")]
fn tui_feedback(
    commands: &[Command],
    feedback: &mut tui_display::host::TerminalFeedback,
) -> std::io::Result<()> {
    for command in commands {
        match command {
            Command::ShowToast { toast } => feedback.status = Some(toast.message.clone()),
            Command::PresentAlert { alert } => {
                feedback.alert = Some((alert.title.clone(), alert.message.clone()));
            }
            Command::OpenExternalUrl { url } => feedback.status = Some(url.clone()),
            Command::ExportFile { file } => {
                // Only the name: the export stays inside the data directory.
                let name = std::path::Path::new(&file.suggested_name)
                    .file_name()
                    .ok_or_else(|| std::io::Error::other("export has no file name"))?;
                let destination = crate::storage::data_dir()?.join(name);
                std::fs::write(&destination, &file.data)?;
                feedback.status = Some(destination.display().to_string());
            }
            Command::ResetApplication => feedback.quit = true,
            _ => {}
        }
    }
    Ok(())
}

#[cfg(any(feature = "cli", feature = "tui"))]
fn terminal_feedback(
    commands: &[Command],
    feedback: &mut crate::terminal::TerminalFeedback,
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

#[cfg(feature = "cli")]
pub fn run_cli() -> Result<(), Box<dyn std::error::Error>> {
    use crate::terminal_cli::{CliTerminal, FdInput};
    let engine = start_shared_engine(false)?;
    let hardware = LinuxHardware::start()?;
    let bridge = HardwareBridge::new(engine, hardware)?;
    let shutdown = crate::terminal_signals::SignalShutdown::new()?;
    let input = FdInput::new(std::io::stdin())?;
    let mut output = std::io::stdout();
    let (columns, rows) = line_terminal_window(&output);
    let mut terminal =
        CliTerminal::new(&mut output, input, terminal_feedback).with_window(columns, rows);
    crate::terminal::run_terminal_with_shutdown(bridge, &mut terminal, || shutdown.requested())?;
    Ok(())
}

/// Piped output has no window: it reports the 80x24 the cli binary
/// reports, so a scripted run sees the same layout every time.
#[cfg(feature = "cli")]
fn line_terminal_window(output: &std::io::Stdout) -> (u16, u16) {
    use std::io::IsTerminal;
    if !output.is_terminal() {
        return (80, 24);
    }
    match rustix::termios::tcgetwinsize(output) {
        Ok(size) if size.ws_col > 0 && size.ws_row > 0 => (size.ws_col, size.ws_row),
        _ => (80, 24),
    }
}

#[cfg(all(test, feature = "gtk"))]
mod tests {
    use super::*;

    // @internal — the app boots even when the hardware runtime cannot start.
    #[test]
    fn the_degraded_hardware_shell_still_starts() {
        let mut hardware = hardware_or_degraded(Err(StartError::Runtime(std::io::Error::other(
            "forced native startup failure",
        ))));
        assert_eq!(
            hardware.report(),
            vauchi_linux_hw::HostDevices::default().report()
        );
        assert_eq!(hardware.pop(), None);
    }
}

#[cfg(all(test, feature = "tui"))]
mod tui_feedback_tests {
    use super::*;
    use tui_display::host::TerminalFeedback;

    // @internal — an effect with no terminal form ended the app (vauchi/private#1233).
    #[test]
    fn an_effect_the_terminal_has_no_form_for_is_left_out() {
        let mut feedback = TerminalFeedback::default();

        let outcome = tui_feedback(&[Command::ReloadTheme], &mut feedback);

        assert!(outcome.is_ok(), "{outcome:?}");
        assert_eq!(feedback.status, None);
        assert_eq!(feedback.alert, None);
        assert!(!feedback.quit);
    }

    // @internal
    #[test]
    fn a_link_to_open_is_shown_and_a_reset_ends_the_app() {
        let mut feedback = TerminalFeedback::default();

        tui_feedback(
            &[
                Command::OpenExternalUrl {
                    url: "https://vauchi.app/help".into(),
                },
                Command::ResetApplication,
            ],
            &mut feedback,
        )
        .unwrap();

        assert_eq!(feedback.status.as_deref(), Some("https://vauchi.app/help"));
        assert!(feedback.quit);
    }
}
