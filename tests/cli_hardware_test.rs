// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg(feature = "cli")]

use std::{
    future::Future,
    io,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};
use vauchi_contract::{Command, Event, ToastSpec};
use vauchi_linux_app::{
    engine::CommandEngine,
    terminal::run_terminal,
    terminal_cli::{CliTerminal, InputChunk, LineInput},
    wiring::HardwareBridge,
};
use vauchi_linux_hw::{
    Backends, Discovery, EventSender, HardwareFailure, HostDevices, LinuxHardware,
};

struct Cancelled(mpsc::Sender<&'static str>);
impl Drop for Cancelled {
    fn drop(&mut self) {
        let _ = self.0.send("cancelled");
    }
}

struct Scan {
    trigger: Mutex<Option<mpsc::Receiver<()>>>,
    ready: mpsc::Sender<()>,
    trace: mpsc::Sender<&'static str>,
}
impl Discovery for Scan {
    fn scan(
        &self,
        _: String,
        mut events: EventSender,
    ) -> Pin<Box<dyn Future<Output = Result<(), HardwareFailure>> + Send + '_>> {
        Box::pin(async move {
            let _cancelled = Cancelled(self.trace.clone());
            self.trigger
                .lock()
                .unwrap()
                .take()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
            events
                .send(Event::HardwareError {
                    transport: "BLE".into(),
                    error: "native callback".into(),
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
            Event::HardwareError { .. } => vec![Command::ShowToast {
                toast: ToastSpec {
                    message: "background observation".into(),
                },
            }],
            _ => Vec::new(),
        }
    }
    fn initial_commands(&self) -> Vec<Command> {
        let fixture: serde_json::Value =
            serde_json::from_str(vauchi_contract::fixtures::presentation_contract_v1_json())
                .unwrap();
        let mut commands: Vec<Command> =
            serde_json::from_value(fixture["initial_commands"].clone()).unwrap();
        commands.push(Command::BleStartScanning {
            service_uuid: "service".into(),
        });
        commands
    }
}

struct Input {
    trigger: Option<mpsc::Sender<()>>,
    ready: mpsc::Receiver<()>,
    observed: Arc<AtomicBool>,
    trace: mpsc::Sender<&'static str>,
    end: u8,
}
impl LineInput for Input {
    fn read(&mut self, timeout: Duration) -> io::Result<InputChunk> {
        assert_eq!(timeout, Duration::from_millis(50));
        if let Some(trigger) = self.trigger.take() {
            assert!(!self.observed.load(Ordering::SeqCst));
            trigger.send(()).unwrap();
            self.ready.recv_timeout(Duration::from_secs(5)).unwrap();
            return Ok(InputChunk::Data(b"1".to_vec()));
        }
        assert!(
            self.observed.load(Ordering::SeqCst),
            "callback must arrive before the line completes"
        );
        match self.end {
            0 => Ok(InputChunk::End),
            1 => Ok(InputChunk::Data(vec![3])),
            _ => Err(io::Error::other("injected input failure")),
        }
    }
    fn restore(&mut self) -> io::Result<()> {
        self.trace.send("restored").unwrap();
        Ok(())
    }
}

// @internal — actual CLI input yields to a real hardware worker, then cleanup covers EOF, quit and I/O errors.
#[test]
fn pending_cli_input_delivers_background_observations_and_cancels_on_exit() {
    for end in [0, 1, 2] {
        let (trigger, receiver) = mpsc::channel();
        let (ready, queued) = mpsc::channel();
        let (trace, recorded) = mpsc::channel();
        let hardware = LinuxHardware::with_backends(
            Backends {
                discovery: Some(Arc::new(Scan {
                    trigger: Mutex::new(Some(receiver)),
                    ready,
                    trace: trace.clone(),
                })),
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
        let observed = Arc::new(AtomicBool::new(false));
        let input = Input {
            trigger: Some(trigger),
            ready: queued,
            observed: observed.clone(),
            trace,
            end,
        };
        let mut output = Vec::new();
        let mut terminal = CliTerminal::new(&mut output, input, |effects, feedback| {
            if !effects.is_empty() {
                assert_eq!(
                    effects,
                    &[Command::ShowToast {
                        toast: ToastSpec {
                            message: "background observation".into()
                        }
                    }]
                );
                observed.store(true, Ordering::SeqCst);
                feedback.status = Some("background observation".into());
            }
            Ok(())
        });
        let result = run_terminal(bridge, &mut terminal);
        if end == 2 {
            assert_eq!(result.unwrap_err().to_string(), "injected input failure");
        } else {
            result.expect("clean terminal exit");
        }
        assert_eq!(
            recorded.recv_timeout(Duration::from_secs(5)).unwrap(),
            "restored"
        );
        assert_eq!(
            recorded.recv_timeout(Duration::from_secs(5)).unwrap(),
            "cancelled"
        );
        drop(terminal);
        assert!(
            String::from_utf8(output)
                .unwrap()
                .contains("background observation")
        );
    }
}
