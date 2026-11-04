// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The commands linux-hw has a port for go to it, not to the renderer
//! (ADR-077). Direct send joined them in private#1029 4d.7c: linux-hw runs
//! the TCP exchange, so the renderer no longer has a TCP adapter.
use vauchi_contract::{Command, DirectEndpoint};
use vauchi_linux_app::wiring::start_hardware_with;
use vauchi_linux_hw::{Backends, HostDevices};

fn ported(command: &Command) -> bool {
    start_hardware_with(
        Backends::default(),
        HostDevices {
            bluetooth_adapter: false,
            sound_card: false,
            video_device: false,
            nfc_reader: false,
        },
    )
    .expect("hardware starts without devices")
    .link
    .execute(command)
}

// @internal
#[test]
fn direct_send_goes_to_the_hardware_shell() {
    assert!(ported(&Command::DirectSend {
        payload: b"card".to_vec(),
        is_initiator: true,
        endpoint: DirectEndpoint::default(),
    }));
    assert!(ported(&Command::BleStopScanning));
    assert!(ported(&Command::AudioStop));
}

// @internal
#[test]
fn a_display_command_stays_with_the_renderer() {
    assert!(!ported(&Command::PerformNativeBack));
}
