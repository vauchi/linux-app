// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{cell::RefCell, rc::Rc};
use vauchi_contract::{BleLinkDirection, Command, Event};
use vauchi_linux_app::{engine::CommandEngine, wiring::HardwareBridge};
use vauchi_linux_hw::LinuxHardware;

struct Engine {
    commands: Vec<Command>,
    events: Rc<RefCell<Vec<Event>>>,
}

impl CommandEngine for Engine {
    fn initial_commands(&self) -> Vec<Command> {
        self.commands.clone()
    }

    fn dispatch(&self, event: Event) -> Vec<Command> {
        let commands = if event == Event::WakeupFired {
            self.commands.clone()
        } else {
            vec![]
        };
        self.events.borrow_mut().push(event);
        commands
    }
}

fn bridge(commands: Vec<Command>) -> (HardwareBridge<Engine>, Rc<RefCell<Vec<Event>>>) {
    let events = Rc::default();
    let engine = Engine {
        commands,
        events: Rc::clone(&events),
    };
    let hardware = LinuxHardware::without_devices().unwrap();
    (HardwareBridge::new(engine, hardware).unwrap(), events)
}

fn unavailable(transport: &str) -> Event {
    Event::HardwareUnavailable {
        transport: transport.into(),
    }
}

// @internal — preserve the removed GTK fallback's transport names for every start/I/O command.
#[test]
fn unsupported_start_and_io_report_their_transport() {
    let cases = [
        (
            Command::AudioEmitChallenge {
                samples: vec![0.0],
                sample_rate: 48000,
            },
            "Audio",
        ),
        (
            Command::AudioListenForResponse {
                timeout_ms: 1000,
                sample_rate: 48000,
            },
            "Audio",
        ),
        (
            Command::BleStartScanning {
                service_uuid: "service".into(),
            },
            "BLE",
        ),
        (
            Command::BleStartAdvertising {
                service_uuid: "service".into(),
                payload: vec![1],
            },
            "BLE",
        ),
        (
            Command::BleConnect {
                device_id: "peer".into(),
            },
            "BLE",
        ),
        (
            Command::BleWriteCharacteristic {
                device_id: "peer".into(),
                direction: BleLinkDirection::Outbound,
                uuid: "characteristic".into(),
                data: vec![1],
            },
            "BLE",
        ),
        (
            Command::BleReadCharacteristic {
                device_id: "peer".into(),
                direction: BleLinkDirection::Outbound,
                uuid: "characteristic".into(),
            },
            "BLE",
        ),
        (
            Command::NfcActivate {
                payload: vec![1],
                apdus: vec![],
            },
            "NFC",
        ),
        (
            Command::NfcSendApdu {
                data: vec![1],
                apdus: vec![],
            },
            "NFC",
        ),
    ];
    for (command, transport) in cases {
        let label = command.clone();
        let (_bridge, events) = bridge(vec![command]);
        assert_eq!(
            &events.borrow()[1..],
            &[unavailable(transport)],
            "{label:?}"
        );
    }
}

// @internal — stopping an absent backend remains a no-op, including during failed exchange cleanup.
#[test]
fn unsupported_stop_commands_do_not_report_unavailable() {
    let (_bridge, events) = bridge(vec![
        Command::AudioStop,
        Command::BleStopScanning,
        Command::BleStopAdvertising,
        Command::BleDisconnect {
            device_id: "peer".into(),
            direction: BleLinkDirection::Outbound,
        },
        Command::NfcDeactivate,
    ]);
    assert_eq!(&events.borrow()[1..], &[]);
}

// @internal — one unavailable report per transport per batch; a later batch must report again.
#[test]
fn unavailable_reports_are_deduplicated_only_within_each_batch() {
    let commands = vec![
        Command::AudioEmitChallenge {
            samples: vec![0.0],
            sample_rate: 48000,
        },
        Command::AudioListenForResponse {
            timeout_ms: 1000,
            sample_rate: 48000,
        },
        Command::BleStartScanning {
            service_uuid: "service".into(),
        },
        Command::BleConnect {
            device_id: "peer".into(),
        },
        Command::NfcActivate {
            payload: vec![1],
            apdus: vec![],
        },
        Command::NfcSendApdu {
            data: vec![1],
            apdus: vec![],
        },
        Command::SwitchCamera { use_front: true },
        Command::SwitchCamera { use_front: false },
    ];
    let expected = vec![
        unavailable("Audio"),
        unavailable("BLE"),
        unavailable("NFC"),
        unavailable("camera_switch"),
    ];
    let (mut bridge, events) = bridge(commands);
    assert_eq!(&events.borrow()[1..], expected);
    bridge.dispatch(Event::WakeupFired).unwrap();
    let mut both = expected.clone();
    both.push(Event::WakeupFired);
    both.extend(expected);
    assert_eq!(&events.borrow()[1..], both);
}
