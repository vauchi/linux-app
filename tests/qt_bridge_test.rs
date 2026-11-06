// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
};
use vauchi_contract::{Command, Event, InputValue, MAX_EVENT_INPUT_VALUE_BYTES};
use vauchi_linux_app::{
    engine::CommandEngine,
    qt::{QtHost, QtHostError},
    wiring::HardwareBridge,
};
use vauchi_linux_hw::{Backends, HostDevices, LinuxHardware};

struct Engine {
    events: Rc<RefCell<Vec<Event>>>,
    invalidated: Rc<Cell<bool>>,
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
    fn take_invalidation(&self) -> bool {
        self.invalidated.replace(false)
    }
}

struct Fixture {
    host: QtHost<Engine>,
    events: Rc<RefCell<Vec<Event>>>,
    invalidated: Rc<Cell<bool>>,
}

fn host(initial: Vec<Command>, reply: Vec<Command>) -> Fixture {
    let events: Rc<RefCell<Vec<Event>>> = Rc::default();
    let invalidated = Rc::new(Cell::new(true));
    let engine = Engine {
        events: events.clone(),
        invalidated: invalidated.clone(),
        initial,
        replies: RefCell::new(VecDeque::from([vec![], reply])),
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
    Fixture {
        host: QtHost::new(HardwareBridge::new(engine, hardware).unwrap()),
        events,
        invalidated,
    }
}

fn batch(host: &mut QtHost<Engine>) -> Option<Vec<Command>> {
    host.poll()
        .unwrap()
        .map(|json| serde_json::from_str(&json).unwrap())
}

// @internal — the Qt host forwards complete batches and repaints without input.
#[test]
fn initial_and_invalidated_commands_are_forwarded_once() {
    let Fixture {
        mut host,
        invalidated,
        ..
    } = host(vec![Command::ResetApplication], vec![]);
    assert_eq!(batch(&mut host), Some(vec![Command::ResetApplication]));
    assert_eq!(batch(&mut host), None);
    invalidated.set(true);
    assert_eq!(batch(&mut host), Some(vec![Command::ResetApplication]));
    assert_eq!(batch(&mut host), None);
}

// @internal — native permission outcomes remain observations across app wiring.
#[test]
fn native_event_and_the_engine_reply_keep_their_payloads() {
    let Fixture {
        mut host, events, ..
    } = host(vec![], vec![Command::ResetApplication]);
    assert_eq!(batch(&mut host), None);
    let event = Event::PermissionDenied {
        transport: "camera".into(),
    };
    assert_eq!(
        host.on_event(&serde_json::to_string(&event).unwrap()),
        Ok(())
    );
    assert_eq!(events.borrow().last(), Some(&event));
    assert_eq!(batch(&mut host), Some(vec![Command::ResetApplication]));
    assert_eq!(batch(&mut host), None);
}

// @internal — the app uses the contract's bounded decoder, not bare serde.
#[test]
fn malformed_and_oversized_native_events_never_reach_the_engine() {
    let Fixture {
        mut host, events, ..
    } = host(vec![], vec![]);
    let before = events.borrow().clone();
    let value = InputValue::Text("x".repeat(MAX_EVENT_INPUT_VALUE_BYTES + 1));
    let oversized_event: Event = serde_json::from_value(serde_json::json!({
        "ValueChanged": {"surface_id": "surface", "binding_id": "binding", "value": value}
    }))
    .unwrap();
    let oversized = serde_json::to_string(&oversized_event).unwrap();
    for raw in [
        "not JSON",
        "{\"UnknownNativeEvent\":{}}",
        oversized.as_str(),
    ] {
        assert_eq!(host.on_event(raw), Err(QtHostError::InvalidEvent));
        assert_eq!(*events.borrow(), before);
        assert_eq!(batch(&mut host), None);
    }
    assert_eq!(
        QtHostError::InvalidEvent.to_string(),
        "Qt supplied an invalid event"
    );
}
