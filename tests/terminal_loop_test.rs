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
}

impl TerminalIo for Console {
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
