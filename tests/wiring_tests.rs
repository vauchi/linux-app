// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later
//! linux-app owns the hardware shell (ADR-078 step 3b): the hardware it
//! hands the GTK display shell reports the host's capabilities first, and
//! takes the commands linux-hw ports.
use vauchi_core::{Command, Event};
use vauchi_linux_app::wiring::start_hardware_with;
use vauchi_linux_hw::{Backends, HostDevices};

fn no_devices() -> HostDevices {
    HostDevices {
        bluetooth_adapter: false,
        sound_card: false,
        video_device: false,
        nfc_reader: false,
    }
}

// @internal
#[test]
fn the_hardware_reports_the_hosts_capabilities_before_anything_else() {
    let started = start_hardware_with(Backends::default(), no_devices())
        .expect("the hardware shell starts with no devices");
    assert!(
        matches!(
            started.capabilities,
            Some(Event::HardwareCapabilitiesReported { .. })
        ),
        "Core hears the capabilities first: {:?}",
        started.capabilities
    );
}

// @internal
#[test]
fn a_ported_command_without_a_device_is_taken_and_reports_nothing() {
    let started = start_hardware_with(Backends::default(), no_devices())
        .expect("the hardware shell starts with no devices");
    assert!(started.link.execute(&Command::NfcDeactivate));
    assert_eq!(started.link.pop(), None);
}
