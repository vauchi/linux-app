// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later
//! The one wiring module (ADR-078 item 3, D-13): builds Core's engine,
//! starts linux-hw's hardware shell and hands both to the display shell.
//! Matching on a `Command` or an `Event` is display or hardware work and
//! belongs in a shell, never here; `lint:no-behaviour` fails the build
//! when either appears outside this file.
use std::cell::RefCell;
use std::rc::Rc;

use gtk_display::core_ui::engine_link::EngineHandle;
use gtk_display::core_ui::hardware_link::{HardwareLink, StartedHardware};
use gtk4::gio;
use gtk4::prelude::*;
use vauchi_app::ui::AppEngine;
use vauchi_contract::Command;
use vauchi_core::Event;
use vauchi_core::settings::NetworkCost;
use vauchi_linux_hw::{Backends, HostDevices, LinuxHardware};

use crate::engine::CoreEngine;

/// Core's engine for the window: storage, the test identity if asked, the
/// system locale, content updates, then the network-cost watcher.
pub fn start_engine() -> EngineHandle {
    let mut vauchi = crate::storage::init_vauchi().expect("Failed to initialize Vauchi backend");
    maybe_seed_test_identity(&mut vauchi);
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
    let engine: EngineHandle = Rc::new(CoreEngine::new(app_engine));
    watch_network_cost(&engine);
    engine
}

/// Creates a test identity so the app skips onboarding and lands on home.
///
/// Triggered by the `--reset-for-testing` flag (the shell strips it before
/// GTK's parser) or `VAUCHI_TEST_SEED=1`, which CI's AT-SPI harness sets.
fn maybe_seed_test_identity(vauchi: &mut vauchi_core::api::Vauchi) {
    let cli_flag = gtk_display::shell::reset_for_testing_requested();
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
fn watch_network_cost(engine: &EngineHandle) {
    let monitor = gio::NetworkMonitor::default();
    report_network_cost(engine, &monitor);
    let changed = engine.clone();
    monitor.connect_network_changed(move |monitor, _| report_network_cost(&changed, monitor));
    let metered = engine.clone();
    monitor.connect_network_metered_notify(move |monitor| report_network_cost(&metered, monitor));
}

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
pub fn start_hardware() -> Option<StartedHardware> {
    let devices = HostDevices::probe();
    start_hardware_with(Backends::host(&devices), devices)
}

/// The hardware shell over the given backends; `None` when its runtime
/// cannot start, and the display shell then keeps every command.
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

struct LinuxHardwareLink(RefCell<LinuxHardware>);

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
