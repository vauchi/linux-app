// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg(feature = "tui")]

use std::{
    future::Future,
    io,
    ops::ControlFlow,
    pin::Pin,
    sync::{Arc, mpsc},
    time::Duration,
};
use tui_display::host::{TerminalIo, run_terminal};
use vauchi_contract::{Command, Event};
use vauchi_linux_app::{engine::CommandEngine, wiring::HardwareBridge};
use vauchi_linux_hw::{
    Backends, Discovery, EventSender, HardwareFailure, HostDevices, LinuxHardware,
};

struct Released(mpsc::Sender<&'static str>);
impl Drop for Released {
    fn drop(&mut self) {
        let _ = self.0.send("cancelled");
    }
}

struct Scan {
    ready: mpsc::Sender<()>,
    released: mpsc::Sender<&'static str>,
}
impl Discovery for Scan {
    fn scan(
        &self,
        _: String,
        mut events: EventSender,
    ) -> Pin<Box<dyn Future<Output = Result<(), HardwareFailure>> + Send + '_>> {
        Box::pin(async move {
            let _released = Released(self.released.clone());
            events
                .send(Event::HardwareError {
                    transport: "BLE".into(),
                    error: "ready".into(),
                })
                .await
                .unwrap();
            self.ready.send(()).unwrap();
            std::future::pending().await
        })
    }
}

struct Engine;
impl CommandEngine for Engine {
    fn dispatch(&self, event: Event) -> Vec<Command> {
        match event {
            Event::HardwareError { .. } => vec![Command::PerformNativeBack],
            _ => vec![],
        }
    }
    fn initial_commands(&self) -> Vec<Command> {
        vec![
            Command::BleStartScanning {
                service_uuid: "service".into(),
            },
            Command::ResetApplication,
        ]
    }
}

fn scanning() -> (HardwareBridge<Engine>, mpsc::Receiver<&'static str>) {
    let (ready, started) = mpsc::channel();
    let (released, cancelled) = mpsc::channel();
    let hardware = LinuxHardware::with_backends(
        Backends {
            discovery: Some(Arc::new(Scan { ready, released })),
            ..Backends::default()
        },
        HostDevices {
            bluetooth_adapter: true,
            sound_card: false,
            video_device: false,
            nfc_reader: false,
        },
    )
    .unwrap();
    let bridge = HardwareBridge::new(Engine, hardware).unwrap();
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    (bridge, cancelled)
}

struct Console {
    render_error: bool,
    input_error: bool,
    batches: Vec<Vec<Command>>,
}
impl TerminalIo for Console {
    fn render(&mut self, commands: &[Command]) -> io::Result<ControlFlow<()>> {
        self.batches.push(commands.to_vec());
        if self.render_error {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        Ok(ControlFlow::Continue(()))
    }
    fn wait_input(&mut self, _: Duration) -> io::Result<ControlFlow<(), Vec<Event>>> {
        if self.input_error {
            return Err(io::ErrorKind::ConnectionReset.into());
        }
        Ok(ControlFlow::Break(()))
    }
}

// @internal — the TUI runs tui-display's loop with this app's bridge as its
// host (private#1234): ready hardware is delivered without typed input, and
// quitting cancels the active worker.
#[test]
fn the_bridge_drives_the_shared_tui_loop_and_quit_cancels_the_worker() {
    let (bridge, cancelled) = scanning();
    let mut terminal = Console {
        render_error: false,
        input_error: false,
        batches: vec![],
    };
    run_terminal(bridge, &mut terminal).unwrap();
    assert_eq!(
        terminal.batches,
        vec![vec![Command::ResetApplication, Command::PerformNativeBack]]
    );
    assert_eq!(
        cancelled.recv_timeout(Duration::from_secs(5)).unwrap(),
        "cancelled"
    );
    assert_eq!(cancelled.try_recv(), Err(mpsc::TryRecvError::Disconnected));
}
