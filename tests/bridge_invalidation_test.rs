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
}

impl CommandEngine for Engine {
    fn dispatch(&self, _: Event) -> Vec<Command> {
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
