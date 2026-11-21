// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded input waiting shared by the terminal display adapters.

use std::{io, ops::ControlFlow, time::Duration};
use vauchi_contract::{Command, Event};

use crate::{engine::CommandEngine, wiring::HardwareBridge};

#[derive(Default)]
pub struct TerminalFeedback {
    pub status: Option<String>,
    pub alert: Option<(String, String)>,
    /// Core wiped its data: the app ends so the next start is a fresh one.
    pub quit: bool,
}

pub trait TerminalIo {
    /// Apply a batch and render it; break when the display requests exit.
    fn render(&mut self, commands: &[Command]) -> io::Result<ControlFlow<()>>;

    /// Return input or exit, waiting no longer than `timeout` when idle.
    /// An empty batch means the wait expired without input.
    fn wait_input(&mut self, timeout: Duration) -> io::Result<ControlFlow<(), Vec<Event>>>;

    /// Restore native terminal state before hardware teardown can block.
    fn restore(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Own the hardware for the loop's lifetime, including error and exit paths.
/// Input adapters must honor the bounded wait so background events progress.
pub fn run_terminal<E: CommandEngine>(
    bridge: HardwareBridge<E>,
    terminal: &mut impl TerminalIo,
) -> io::Result<()> {
    run_terminal_with_shutdown(bridge, terminal, || false)
}

/// Honor shutdown between bounded waits and restore the terminal before hardware teardown.
pub fn run_terminal_with_shutdown<E: CommandEngine>(
    mut bridge: HardwareBridge<E>,
    terminal: &mut impl TerminalIo,
    shutdown_requested: impl Fn() -> bool,
) -> io::Result<()> {
    let mut guard = RestoreTerminal {
        terminal,
        active: true,
    };
    let result = run_loop(&mut bridge, guard.terminal, shutdown_requested);
    guard.active = false;
    let restored = guard.terminal.restore();
    result.and(restored)
}

struct RestoreTerminal<'a, T: TerminalIo> {
    terminal: &'a mut T,
    active: bool,
}

impl<T: TerminalIo> Drop for RestoreTerminal<'_, T> {
    fn drop(&mut self) {
        if self.active {
            let _ = self.terminal.restore();
        }
    }
}

fn run_loop<E: CommandEngine>(
    bridge: &mut HardwareBridge<E>,
    terminal: &mut impl TerminalIo,
    shutdown_requested: impl Fn() -> bool,
) -> io::Result<()> {
    // Core schedules its next wakeup only when one fires (ADR-044 Am2a):
    // the host fires the first, or no heartbeat ever runs.
    bridge
        .dispatch(Event::WakeupFired)
        .map_err(io::Error::other)?;
    while !shutdown_requested() {
        bridge.poll().map_err(io::Error::other)?;
        let commands = bridge.take_commands();
        if !commands.is_empty() {
            let rendered = terminal.render(&commands);
            if shutdown_requested() || rendered?.is_break() {
                return Ok(());
            }
        }
        let input = terminal.wait_input(Duration::from_millis(50));
        if shutdown_requested() {
            return Ok(());
        }
        match input? {
            ControlFlow::Break(()) => return Ok(()),
            ControlFlow::Continue(events) => {
                for event in events {
                    bridge.dispatch(event).map_err(io::Error::other)?;
                }
            }
        }
    }
    Ok(())
}
