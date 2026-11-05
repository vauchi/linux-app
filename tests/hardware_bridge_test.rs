// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    cell::RefCell,
    collections::VecDeque,
    rc::Rc,
    sync::{Arc, mpsc},
    time::Duration,
};
use vauchi_contract::{Command, DirectEndpoint, Event, executor::DeliveryError};
use vauchi_linux_app::{engine::CommandEngine, wiring::HardwareBridge};
use vauchi_linux_hw::{Backends, DirectBackend, ExecuteError, HostDevices, LinuxHardware};

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

fn engine(initial: Vec<Command>, reply: Vec<Command>) -> Engine {
    Engine {
        events: Rc::default(),
        initial,
        replies: RefCell::new(VecDeque::from([vec![], reply])),
    }
}

fn hardware(backends: Backends) -> LinuxHardware {
    LinuxHardware::with_backends(
        backends,
        HostDevices {
            bluetooth_adapter: false,
            sound_card: false,
            video_device: false,
            nfc_reader: false,
        },
    )
    .unwrap()
}

struct Direct(mpsc::Sender<(DirectEndpoint, Vec<u8>, bool)>);

impl DirectBackend for Direct {
    fn exchange(
        &self,
        endpoint: &DirectEndpoint,
        payload: &[u8],
        initiator: bool,
    ) -> Result<Vec<u8>, String> {
        self.0
            .send((endpoint.clone(), payload.to_vec(), initiator))
            .unwrap();
        Ok(b"peer payload".to_vec())
    }
}

// @internal — app wiring reports the hardware facts to Core once.
#[test]
fn startup_reports_exact_capabilities_and_keeps_display_commands() {
    let hardware = hardware(Backends::default());
    let report = hardware.report();
    let engine = engine(vec![Command::PerformNativeBack], vec![]);
    let events = engine.events.clone();
    let mut bridge = HardwareBridge::new(engine, hardware).unwrap();
    assert_eq!(*events.borrow(), vec![report]);
    assert_eq!(bridge.take_commands(), vec![Command::PerformNativeBack]);
    assert_eq!(bridge.take_commands(), vec![]);
    assert!(!bridge.poll().unwrap());
    assert_eq!(events.borrow().len(), 1);
}

// @internal — commands keep their payload, endpoint and role across the app.
#[test]
fn hardware_observations_reach_core_without_waiting_for_input() {
    for initiator in [false, true] {
        let (sent, received) = mpsc::channel();
        let hardware = hardware(Backends {
            direct: Some(Arc::new(Direct(sent))),
            ..Backends::default()
        });
        let report = hardware.report();
        let endpoint = DirectEndpoint {
            port: 19284,
            fallback_host: "127.0.0.2".into(),
            ..DirectEndpoint::default()
        };
        let engine = engine(
            vec![Command::DirectSend {
                payload: b"local payload".to_vec(),
                is_initiator: initiator,
                endpoint: endpoint.clone(),
            }],
            vec![Command::PerformNativeBack],
        );
        let events = engine.events.clone();
        let mut bridge = HardwareBridge::new(engine, hardware).unwrap();
        assert_eq!(bridge.take_commands(), vec![]);
        assert_eq!(
            received.recv_timeout(Duration::from_secs(5)).unwrap(),
            (endpoint, b"local payload".to_vec(), initiator)
        );
        assert!(bridge.wait_for_event(Duration::from_secs(5)).unwrap());
        assert_eq!(
            *events.borrow(),
            vec![
                report,
                Event::DirectPayloadReceived {
                    data: b"peer payload".to_vec()
                }
            ]
        );
        assert_eq!(bridge.take_commands(), vec![Command::PerformNativeBack]);
    }
}

// @internal — a platform input is forwarded unchanged before executing its reply.
#[test]
fn input_events_and_display_replies_are_forwarded_unchanged() {
    let engine = engine(vec![], vec![Command::PerformNativeBack]);
    let events = engine.events.clone();
    let hardware = hardware(Backends::default());
    let report = hardware.report();
    let mut bridge = HardwareBridge::new(engine, hardware).unwrap();
    bridge.dispatch(Event::WakeupFired).unwrap();
    assert_eq!(*events.borrow(), vec![report, Event::WakeupFired]);
    assert_eq!(bridge.take_commands(), vec![Command::PerformNativeBack]);
}

// @internal — linux-hw owns support decisions; unsupported ports emit no event.
#[test]
fn unsupported_hardware_does_not_consume_display_commands() {
    let engine = engine(
        vec![Command::NfcDeactivate, Command::PerformNativeBack],
        vec![],
    );
    let events = engine.events.clone();
    let mut bridge = HardwareBridge::new(engine, hardware(Backends::default())).unwrap();
    assert_eq!(bridge.take_commands(), vec![Command::PerformNativeBack]);
    assert!(!bridge.wait_for_event(Duration::ZERO).unwrap());
    assert_eq!(events.borrow().len(), 1);
}

// @internal — delivery errors are returned to the host, never silently dropped.
#[test]
fn hardware_delivery_errors_reach_the_host() {
    let (sent, _received) = mpsc::channel();
    let engine = engine(
        vec![],
        vec![
            Command::DirectSend {
                payload: vec![],
                is_initiator: true,
                endpoint: DirectEndpoint::default(),
            },
            Command::PerformNativeBack,
        ],
    );
    let hardware = hardware(Backends {
        direct: Some(Arc::new(Direct(sent))),
        ..Backends::default()
    });
    let mut bridge = HardwareBridge::new(engine, hardware).unwrap();
    assert_eq!(
        bridge.dispatch(Event::WakeupFired),
        Err(ExecuteError::Delivery(DeliveryError::PayloadTooLarge))
    );
    assert_eq!(bridge.take_commands(), vec![Command::PerformNativeBack]);
}
