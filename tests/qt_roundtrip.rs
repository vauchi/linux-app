// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{cell::RefCell, rc::Rc, sync::mpsc, time::Duration};
use vauchi_contract::{Command, Event};
use vauchi_linux_app::{engine::CommandEngine, wiring::HardwareBridge};
use vauchi_linux_hw::{Backends, HostDevices, LinuxHardware};

struct Engine(Rc<RefCell<Vec<Event>>>);
impl CommandEngine for Engine {
    fn dispatch(&self, event: Event) -> Vec<Command> {
        let complete =
            matches!(&event, Event::HardwareUnavailable { transport } if transport == "BLE");
        self.0.borrow_mut().push(event);
        if complete {
            qt_display::stop();
        }
        vec![]
    }
    fn initial_commands(&self) -> Vec<Command> {
        vec![Command::RequestPermission {
            transport: "BLE".into(),
        }]
    }
}

// @internal — the actual Qt library returns native effects through the app bridge.
fn main() {
    let observations = Rc::new(RefCell::new(vec![]));
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
    let bridge = HardwareBridge::new(Engine(observations.clone()), hardware).unwrap();
    let (done, completed) = mpsc::channel();
    let watchdog = std::thread::spawn(move || {
        if completed.recv_timeout(Duration::from_secs(10)).is_err() {
            qt_display::stop();
        }
    });
    let args = vec![
        "qt-app-roundtrip".into(),
        "-platform".into(),
        "offscreen".into(),
    ];
    let result = vauchi_linux_app::qt::run(&args, bridge);
    let _ = done.send(());
    watchdog.join().unwrap();
    assert_eq!(result, Ok(0));
    let outcomes: Vec<_> = observations
        .borrow()
        .iter()
        .filter(|event| matches!(event, Event::HardwareUnavailable { .. }))
        .cloned()
        .collect();
    assert_eq!(
        outcomes,
        vec![Event::HardwareUnavailable {
            transport: "BLE".into()
        }]
    );
    assert_eq!(Rc::strong_count(&observations), 1);
}
