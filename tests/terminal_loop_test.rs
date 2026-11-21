// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    io,
    ops::ControlFlow,
    rc::Rc,
    time::Duration,
};
use vauchi_contract::{Command, Event};
use vauchi_linux_app::{
    engine::CommandEngine,
    terminal::{TerminalIo, run_terminal},
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
        vec![]
    }
    fn initial_commands(&self) -> Vec<Command> {
        self.batches.borrow_mut().pop_front().unwrap_or_default()
    }
    fn take_invalidation(&self) -> bool {
        self.invalidated.replace(false)
    }
}

fn bridge(invalidated: Rc<Cell<bool>>, events: Rc<RefCell<Vec<Event>>>) -> HardwareBridge<Engine> {
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
    HardwareBridge::new(
        Engine {
            invalidated,
            events,
            batches: RefCell::new(VecDeque::from([
                vec![Command::PerformNativeBack],
                vec![Command::ResetApplication],
            ])),
        },
        hardware,
    )
    .unwrap()
}

struct Console {
    invalidated: Rc<Cell<bool>>,
    reads: VecDeque<io::Result<ControlFlow<(), Vec<Event>>>>,
    batches: Vec<Vec<Command>>,
    waits: Vec<Duration>,
    stop_after_render: bool,
    restored: Rc<Cell<bool>>,
}

impl TerminalIo for Console {
    fn restore(&mut self) -> io::Result<()> {
        self.restored.set(true);
        Ok(())
    }
    fn render(&mut self, commands: &[Command]) -> io::Result<ControlFlow<()>> {
        self.batches.push(commands.to_vec());
        Ok(if self.stop_after_render {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        })
    }
    fn wait_input(&mut self, timeout: Duration) -> io::Result<ControlFlow<(), Vec<Event>>> {
        self.waits.push(timeout);
        self.invalidated.set(true);
        self.reads
            .pop_front()
            .expect("host must stop after input ends")
    }
}

fn console(
    invalidated: Rc<Cell<bool>>,
    reads: Vec<io::Result<ControlFlow<(), Vec<Event>>>>,
) -> Console {
    Console {
        invalidated,
        reads: reads.into(),
        batches: vec![],
        waits: vec![],
        stop_after_render: false,
        restored: Rc::default(),
    }
}

// @internal — waiting for terminal input must not prevent background presentation delivery.
#[test]
fn idle_input_delivers_a_background_repaint_before_waiting_again() {
    let invalidated = Rc::new(Cell::new(true));
    let mut terminal = console(
        invalidated.clone(),
        vec![
            Ok(ControlFlow::Continue(vec![])),
            Ok(ControlFlow::Break(())),
        ],
    );
    run_terminal(bridge(invalidated, Rc::default()), &mut terminal).unwrap();
    assert_eq!(
        terminal.batches,
        vec![
            vec![Command::PerformNativeBack],
            vec![Command::ResetApplication]
        ]
    );
    assert_eq!(terminal.waits, vec![Duration::from_millis(50); 2]);
}

// @internal — a terminal host forwards input unchanged and quits without another read.
#[test]
fn input_is_dispatched_unchanged_and_quit_stops_the_loop() {
    let invalidated = Rc::new(Cell::new(true));
    let events = Rc::new(RefCell::new(vec![]));
    let mut terminal = console(
        invalidated.clone(),
        vec![
            Ok(ControlFlow::Continue(vec![Event::WakeupFired])),
            Ok(ControlFlow::Break(())),
        ],
    );
    run_terminal(bridge(invalidated, events.clone()), &mut terminal).unwrap();
    assert_eq!(events.borrow().last(), Some(&Event::WakeupFired));
    assert_eq!(terminal.waits.len(), 2);
}

// @internal — Core schedules its next wakeup only when one fires, so without
// the host's first one no heartbeat runs: no sync, and a device link stays on
// "Generating link..." (vauchi/private#1297).
#[test]
fn the_host_fires_the_first_wakeup_once_before_it_waits_for_input() {
    let invalidated = Rc::new(Cell::new(true));
    let events = Rc::new(RefCell::new(vec![]));
    let mut terminal = console(invalidated.clone(), vec![Ok(ControlFlow::Break(()))]);

    run_terminal(bridge(invalidated, events.clone()), &mut terminal).unwrap();

    let wakeups = events
        .borrow()
        .iter()
        .filter(|event| **event == Event::WakeupFired)
        .count();
    assert_eq!(wakeups, 1);
    assert_eq!(terminal.waits.len(), 1);
}

// @internal — input failure must return promptly instead of spinning or retrying EOF.
#[test]
fn input_failure_ends_the_loop_and_preserves_its_error_kind() {
    let invalidated = Rc::new(Cell::new(true));
    let mut terminal = console(
        invalidated.clone(),
        vec![Err(io::Error::from(io::ErrorKind::BrokenPipe))],
    );
    let error = run_terminal(bridge(invalidated, Rc::default()), &mut terminal).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    assert_eq!(terminal.waits.len(), 1);
}

// @internal — native terminal completion must not enter an input wait.
#[test]
fn display_completion_stops_before_reading_input() {
    let invalidated = Rc::new(Cell::new(true));
    let mut terminal = console(invalidated.clone(), vec![]);
    terminal.stop_after_render = true;
    run_terminal(bridge(invalidated, Rc::default()), &mut terminal).unwrap();
    assert_eq!(terminal.batches, vec![vec![Command::PerformNativeBack]]);
    assert_eq!(terminal.waits, vec![]);
}

struct TeardownProbe {
    restored: Rc<Cell<bool>>,
    observed: Rc<Cell<Option<bool>>>,
}
impl Drop for TeardownProbe {
    fn drop(&mut self) {
        self.observed.set(Some(self.restored.get()));
    }
}
impl CommandEngine for TeardownProbe {
    fn dispatch(&self, _: Event) -> Vec<Command> {
        vec![]
    }
    fn initial_commands(&self) -> Vec<Command> {
        vec![]
    }
}

// @internal — restoring the console must precede hardware teardown, which may wait on a native backend.
#[test]
fn terminal_is_restored_before_the_owned_bridge_is_destroyed() {
    for case in ["quit", "io-error", "panic"] {
        let reads = match case {
            "quit" => vec![Ok(ControlFlow::Break(()))],
            "io-error" => vec![Err(io::ErrorKind::BrokenPipe.into())],
            _ => vec![],
        };
        let mut terminal = console(Rc::default(), reads);
        let observed = Rc::new(Cell::new(None));
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
        let bridge = HardwareBridge::new(
            TeardownProbe {
                restored: terminal.restored.clone(),
                observed: observed.clone(),
            },
            hardware,
        )
        .unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_terminal(bridge, &mut terminal)
        }));
        let outcome = match result {
            Ok(Ok(())) => "quit",
            Ok(Err(error)) => {
                assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
                "io-error"
            }
            Err(_) => "panic",
        };
        assert_eq!(outcome, case);
        assert_eq!(observed.get(), Some(true));
    }
}
