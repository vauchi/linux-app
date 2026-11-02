// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: AGPL-3.0-or-later
//! The one wiring module (ADR-078 item 3, D-13): starts linux-hw's
//! hardware shell and hands it to the display shell. Matching on a
//! `Command` or an `Event` is display or hardware work and belongs in a
//! shell, never here; `lint:no-behaviour` fails the build otherwise.
use std::rc::Rc;

use gtk_display::core_ui::hardware_link::StartedHardware;
use gtk_display::platform::linux_hardware_link::LinuxHardwareLink;
use vauchi_linux_hw::{Backends, HostDevices, LinuxHardware};

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
