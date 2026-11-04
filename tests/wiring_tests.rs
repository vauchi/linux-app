// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later
//! linux-app owns the hardware shell (ADR-078 step 3b): the hardware it
//! hands the GTK display shell reports the host's capabilities first, and
//! takes the commands linux-hw ports.
use std::sync::{Arc, mpsc};
use std::time::Duration;
use vauchi_contract::DirectEndpoint;
use vauchi_core::{Command, Event};
use vauchi_linux_app::wiring::start_hardware_with;
use vauchi_linux_hw::DirectBackend;
use vauchi_linux_hw::{Backends, HostDevices};

struct RecordingDirect(mpsc::Sender<(DirectEndpoint, Vec<u8>, bool)>);

impl DirectBackend for RecordingDirect {
    fn exchange(
        &self,
        endpoint: &DirectEndpoint,
        payload: &[u8],
        is_initiator: bool,
    ) -> Result<Vec<u8>, String> {
        self.0
            .send((endpoint.clone(), payload.to_vec(), is_initiator))
            .map_err(|error| error.to_string())?;
        Ok(b"peer card".to_vec())
    }
}

// @internal — ADR-078 app wiring preserves Core's direct-send request.
#[test]
fn direct_send_reaches_the_app_hardware_backend() {
    for is_initiator in [false, true] {
        let (sent, received) = mpsc::channel();
        let started = start_hardware_with(
            Backends {
                direct: Some(Arc::new(RecordingDirect(sent))),
                ..Backends::default()
            },
            no_devices(),
        )
        .expect("hardware starts with the supplied direct backend");
        let endpoint = DirectEndpoint {
            port: 19284,
            fallback_host: "127.0.0.2".into(),
            ..DirectEndpoint::default()
        };
        let payload = b"local card".to_vec();
        assert!(started.link.execute(&Command::DirectSend {
            payload: payload.clone(),
            is_initiator,
            endpoint: endpoint.clone(),
        }));
        assert_eq!(
            received
                .recv_timeout(Duration::from_secs(5))
                .expect("direct backend receives the request"),
            (endpoint, payload, is_initiator),
        );
    }
}

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
