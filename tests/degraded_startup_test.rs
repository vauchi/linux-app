// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg(feature = "gtk")]

use gtk_display::core_ui::engine_link::EngineLink;
use std::{cell::RefCell, rc::Rc};
use vauchi_contract::{Command, Event};
use vauchi_linux_app::{engine::CommandEngine, wiring::BridgeEngine};
use vauchi_linux_hw::{HostDevices, LinuxHardware, StartError};

struct Engine(Rc<RefCell<Vec<Event>>>);

impl CommandEngine for Engine {
    fn dispatch(&self, event: Event) -> Vec<Command> {
        self.0.borrow_mut().push(event);
        vec![]
    }

    fn initial_commands(&self) -> Vec<Command> {
        vec![Command::PerformNativeBack]
    }
}

// @internal — persistent native startup failure must not abort the display bridge.
#[test]
fn the_display_bridge_survives_persistent_native_startup_failure() {
    const CHILD: &str = "VAUCHI_TEST_APP_FD_EXHAUSTION";
    if std::env::var_os(CHILD).is_some() {
        let mut held_files = Vec::new();
        let exhausted = loop {
            match std::fs::File::open("/dev/null") {
                Ok(file) => held_files.push(file),
                Err(error) => break error,
            }
        };
        let started = LinuxHardware::start();
        match &started {
            Err(StartError::Runtime(error)) => {
                assert_eq!(error.raw_os_error(), exhausted.raw_os_error());
            }
            Err(error) => panic!("wrong native startup failure: {error}"),
            Ok(_) => panic!("native runtime started with no file descriptors"),
        }
        let events = Rc::new(RefCell::new(Vec::new()));
        let bridge = BridgeEngine::from_hardware_start(Engine(events.clone()), started).unwrap();
        assert_eq!(
            events.borrow().as_slice(),
            &[HostDevices::default().report()]
        );
        assert_eq!(bridge.initial_commands(), vec![Command::PerformNativeBack]);
        assert!(bridge.take_invalidation(), "the boot repaint is preserved");
        assert!(!bridge.take_invalidation());
        return;
    }

    let output = std::process::Command::new("sh")
        .args([
            "-ec",
            "ulimit -n 64; exec \"$1\" --exact the_display_bridge_survives_persistent_native_startup_failure --nocapture",
            "display-startup-test",
        ])
        .arg(std::env::current_exe().unwrap())
        .env(CHILD, "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "isolated startup failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
