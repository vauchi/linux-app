// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
    time::Duration,
};
use vauchi_app::ui::AppEngine;
use vauchi_contract::{Command, Event};
use vauchi_core::api::Vauchi;
use vauchi_linux_app::{
    engine::{CommandEngine, CoreEngine},
    wiring::HardwareBridge,
};
use vauchi_linux_hw::{Backends, HostDevices, LinuxHardware};

struct Engine {
    invalidated: Rc<Cell<bool>>,
    batches: RefCell<VecDeque<Vec<Command>>>,
    events: Rc<RefCell<Vec<Event>>>,
}

impl CommandEngine for Engine {
    fn dispatch(&self, event: Event) -> Vec<Command> {
        self.events.borrow_mut().push(event);
        Vec::new()
    }
    fn initial_commands(&self) -> Vec<Command> {
        self.batches.borrow_mut().pop_front().unwrap_or_default()
    }
    fn take_invalidation(&self) -> bool {
        self.invalidated.replace(false)
    }
}

fn bridge() -> (HardwareBridge<Engine>, Rc<Cell<bool>>) {
    let invalidated = Rc::new(Cell::new(true));
    let engine = Engine {
        invalidated: invalidated.clone(),
        events: Rc::default(),
        batches: RefCell::new(VecDeque::from([
            vec![],
            vec![Command::PerformNativeBack],
            vec![Command::ResetApplication],
        ])),
    };
    let hardware = LinuxHardware::with_backends(
        Backends::default(),
        HostDevices {
            bluetooth_adapter: false,
            sound_card: false,
            video_device: false,
            nfc_reader: false,
        },
    )
    .unwrap();
    (HardwareBridge::new(engine, hardware).unwrap(), invalidated)
}

// @internal — coalesced Core invalidations must repaint without terminal input.
#[test]
fn polling_delivers_invalidated_batches_once_without_an_input_or_hardware_event() {
    let (mut bridge, invalidated) = bridge();
    assert!(
        !bridge.poll().unwrap(),
        "startup invalidation is covered by the initial batch"
    );
    assert_eq!(bridge.take_commands(), vec![]);
    invalidated.set(true);
    invalidated.set(true);
    assert!(bridge.poll().unwrap());
    assert_eq!(bridge.take_commands(), vec![Command::PerformNativeBack]);
    assert!(!bridge.poll().unwrap());
    assert_eq!(bridge.take_commands(), vec![]);
    invalidated.set(true);
    assert!(bridge.poll().unwrap());
    assert_eq!(bridge.take_commands(), vec![Command::ResetApplication]);
}

// @internal — a waiting host must process already-ready repaint work before blocking.
#[test]
fn waiting_delivers_ready_invalidations_and_reports_idle_after_consuming_them() {
    let (mut bridge, invalidated) = bridge();
    assert!(!bridge.wait_for_event(Duration::ZERO).unwrap());
    invalidated.set(true);
    assert!(bridge.wait_for_event(Duration::ZERO).unwrap());
    assert_eq!(bridge.take_commands(), vec![Command::PerformNativeBack]);
    assert!(!bridge.wait_for_event(Duration::ZERO).unwrap());
    assert_eq!(bridge.take_commands(), vec![]);
}

// @internal — the generic engine trait must preserve Core's invalidation subscription.
#[test]
fn core_trait_forwards_and_consumes_the_initial_invalidation() {
    let engine = CoreEngine::new(AppEngine::new(Vauchi::in_memory().unwrap()));
    assert!(CommandEngine::take_invalidation(&engine));
    assert!(!CoreEngine::take_invalidation(&engine));
}

struct ReadyDiscovery(std::sync::mpsc::Sender<()>);

impl vauchi_linux_hw::Discovery for ReadyDiscovery {
    fn scan(
        &self,
        _: String,
        mut events: vauchi_linux_hw::EventSender,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<(), vauchi_linux_hw::HardwareFailure>>
                + Send
                + '_,
        >,
    > {
        Box::pin(async move {
            events
                .send(Event::HardwareError {
                    transport: "BLE".into(),
                    error: "ready observation".into(),
                })
                .await
                .unwrap();
            self.0.send(()).unwrap();
            std::future::pending().await
        })
    }
}

fn hardware_with_ready_observation() -> LinuxHardware {
    use std::sync::Arc;

    let (sent, received) = std::sync::mpsc::channel();
    let mut hardware = LinuxHardware::with_backends(
        Backends {
            discovery: Some(Arc::new(ReadyDiscovery(sent))),
            ..Backends::default()
        },
        HostDevices::default(),
    )
    .unwrap();
    hardware
        .execute(Command::BleStartScanning {
            service_uuid: "test-service".into(),
        })
        .unwrap();
    received.recv_timeout(Duration::from_secs(5)).unwrap();
    hardware
}

// @internal — continuous repaints must not starve already-ready hardware observations.
#[test]
fn waiting_processes_ready_hardware_even_when_a_repaint_is_pending() {
    let invalidated = Rc::new(Cell::new(true));
    let events = Rc::new(RefCell::new(Vec::new()));
    let engine = Engine {
        invalidated: invalidated.clone(),
        events: events.clone(),
        batches: RefCell::default(),
    };
    let mut bridge = HardwareBridge::new(engine, hardware_with_ready_observation()).unwrap();
    events.borrow_mut().clear();
    invalidated.set(true);
    assert!(bridge.wait_for_event(Duration::ZERO).unwrap());
    assert_eq!(
        *events.borrow(),
        vec![Event::HardwareError {
            transport: "BLE".into(),
            error: "ready observation".into(),
        }]
    );
}
