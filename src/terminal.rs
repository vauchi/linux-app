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
    mut bridge: HardwareBridge<E>,
    terminal: &mut impl TerminalIo,
) -> io::Result<()> {
    let mut guard = RestoreTerminal {
        terminal,
        active: true,
    };
    let result = run_loop(&mut bridge, guard.terminal);
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
) -> io::Result<()> {
    loop {
        bridge.poll().map_err(io::Error::other)?;
        let commands = bridge.take_commands();
        if !commands.is_empty() && terminal.render(&commands)?.is_break() {
            return Ok(());
        }
        match terminal.wait_input(Duration::from_millis(50))? {
            ControlFlow::Break(()) => return Ok(()),
            ControlFlow::Continue(events) => {
                for event in events {
                    bridge.dispatch(event).map_err(io::Error::other)?;
                }
            }
        }
    }
}
