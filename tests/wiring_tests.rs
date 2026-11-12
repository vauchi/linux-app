// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

//! linux-app owns the hardware shell (ADR-078 step 3b): the bridge reports
//! the host's capabilities to Core first and routes the commands linux-hw
//! ports, so no display shell sees them.
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::{Arc, mpsc};
use std::time::Duration;
use vauchi_contract::{Command, DirectEndpoint, Event};
use vauchi_linux_app::{engine::CommandEngine, wiring::HardwareBridge};
use vauchi_linux_hw::DirectBackend;
use vauchi_linux_hw::{Backends, HostDevices};

struct Engine {
    events: Rc<RefCell<Vec<Event>>>,
    initial: Vec<Command>,
    replies: RefCell<VecDeque<Vec<Command>>>,
}

impl CommandEngine for Engine {
    fn dispatch(&self, event: Event) -> Vec<Command> {
        self.events.borrow_mut().push(event);
        self.replies.borrow_mut().pop_front().unwrap_or_default()
    }

    fn initial_commands(&self) -> Vec<Command> {
        self.initial.clone()
    }
}

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

fn no_devices() -> HostDevices {
    HostDevices {
        bluetooth_adapter: false,
        sound_card: false,
        video_device: false,
        nfc_reader: false,
    }
}

// @internal — ADR-078 app wiring preserves Core's direct-send request.
#[test]
fn direct_send_reaches_the_app_hardware_backend() {
    for is_initiator in [false, true] {
        let (sent, received) = mpsc::channel();
        let endpoint = DirectEndpoint {
            port: 19284,
            fallback_host: "127.0.0.2".into(),
            ..DirectEndpoint::default()
        };
        let payload = b"local card".to_vec();
        let engine = Engine {
            events: Rc::default(),
            initial: vec![Command::DirectSend {
                payload: payload.clone(),
                is_initiator,
                endpoint: endpoint.clone(),
            }],
            replies: RefCell::new(VecDeque::new()),
        };
        let hardware = vauchi_linux_hw::LinuxHardware::with_backends(
            Backends {
                direct: Some(Arc::new(RecordingDirect(sent))),
                ..Backends::default()
            },
            no_devices(),
        )
        .expect("hardware starts with the supplied direct backend");
        let mut bridge = HardwareBridge::new(engine, hardware).unwrap();
        assert_eq!(bridge.take_commands(), vec![]);
        assert_eq!(
            received
                .recv_timeout(Duration::from_secs(5))
                .expect("direct backend receives the request"),
            (endpoint, payload, is_initiator),
        );
    }
}

// @internal — the capabilities report is the first event Core hears.
#[test]
fn the_hardware_reports_the_hosts_capabilities_before_anything_else() {
    let engine = Engine {
        events: Rc::default(),
        initial: vec![],
        replies: RefCell::new(VecDeque::new()),
    };
    let events = engine.events.clone();
    let hardware = vauchi_linux_hw::LinuxHardware::with_backends(Backends::default(), no_devices())
        .expect("the hardware shell starts with no devices");
    let _bridge = HardwareBridge::new(engine, hardware).unwrap();
    assert!(
        matches!(
            events.borrow().first(),
            Some(Event::HardwareCapabilitiesReported { .. })
        ),
        "Core hears the capabilities first: {:?}",
        events.borrow()
    );
}

// @internal — a ported command without a device is taken and reports nothing.
#[test]
fn a_ported_command_without_a_device_is_taken_and_reports_nothing() {
    let engine = Engine {
        events: Rc::default(),
        initial: vec![Command::NfcDeactivate],
        replies: RefCell::new(VecDeque::new()),
    };
    let events = engine.events.clone();
    let hardware = vauchi_linux_hw::LinuxHardware::with_backends(Backends::default(), no_devices())
        .expect("the hardware shell starts with no devices");
    let mut bridge = HardwareBridge::new(engine, hardware).unwrap();
    assert_eq!(bridge.take_commands(), vec![]);
    assert_eq!(bridge.pop_event(), None);
    assert_eq!(events.borrow().len(), 1);
}
