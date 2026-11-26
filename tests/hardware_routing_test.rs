// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The app-owned bridge routes hardware commands to linux-hw, and the
//! engine the GTK shell sees returns only display commands (#1252):
//! the renderer has no hardware fallback to catch them.
use gtk_display::core_ui::engine_link::EngineLink;
use std::{cell::RefCell, collections::VecDeque};
use vauchi_contract::{Command, Event};
use vauchi_linux_app::{engine::CommandEngine, wiring::BridgeEngine};
use vauchi_linux_hw::{Backends, HostDevices, LinuxHardware};

struct Engine {
    initial: Vec<Command>,
    replies: RefCell<VecDeque<Vec<Command>>>,
}

impl CommandEngine for Engine {
    fn dispatch(&self, _event: Event) -> Vec<Command> {
        self.replies.borrow_mut().pop_front().unwrap_or_default()
    }

    fn initial_commands(&self) -> Vec<Command> {
        self.initial.clone()
    }
}

fn engine(initial: Vec<Command>, reply: Vec<Command>) -> Engine {
    Engine {
        initial,
        replies: RefCell::new(VecDeque::from([vec![], reply])),
    }
}

fn hardware() -> LinuxHardware {
    LinuxHardware::with_backends(
        Backends::default(),
        HostDevices {
            bluetooth_adapter: false,
            sound_card: false,
            video_device: false,
            nfc_reader: false,
        },
    )
    .unwrap()
}

// @internal — Core's boot batch reaches the renderer without its hardware commands.
#[test]
fn initial_commands_return_only_display_commands() {
    let engine = engine(
        vec![Command::NfcDeactivate, Command::PerformNativeBack],
        vec![],
    );
    let bridge = BridgeEngine::new(engine, hardware()).unwrap();
    assert_eq!(bridge.initial_commands(), vec![Command::PerformNativeBack]);
}

// @internal — a display command crosses the app with its payload unchanged.
#[test]
fn display_command_reaches_the_renderer_unchanged() {
    let command = Command::QrDisplay {
        data: "vauchi://exchange/abc123".into(),
    };
    let engine = engine(vec![command.clone()], vec![]);
    let bridge = BridgeEngine::new(engine, hardware()).unwrap();
    assert_eq!(bridge.initial_commands(), vec![command]);
}

// @internal — a dispatch's hardware commands are executed, never returned to the renderer.
#[test]
fn dispatch_returns_only_display_commands() {
    let engine = engine(
        vec![],
        vec![Command::BleStopScanning, Command::PerformNativeBack],
    );
    let bridge = BridgeEngine::new(engine, hardware()).unwrap();
    let commands = bridge.dispatch(Event::WakeupFired);
    assert_eq!(commands, vec![Command::PerformNativeBack]);
}

// @internal — the bridge consumed Core's initial invalidation token, so the
// first poll forces the boot repaint; after it, the subscription drives.
#[test]
fn invalidation_forwards_to_core() {
    let engine = engine(vec![], vec![]);
    let bridge = BridgeEngine::new(engine, hardware()).unwrap();
    assert!(bridge.take_invalidation(), "the boot repaint still happens");
    assert!(!bridge.take_invalidation());
}
