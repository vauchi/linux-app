// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Line-oriented CLI display with bounded native input waits.

use crate::terminal::{TerminalFeedback, TerminalIo};
use cli_display::presentation::{LinePrompt, OfferedInputs, PresentationState, render_to_string};
use std::{
    collections::VecDeque,
    io::{self, Write},
    ops::ControlFlow,
    time::Duration,
};
use vauchi_contract::{Command, Event};

mod input;
pub use input::{FdInput, InputChunk, LineInput};

pub struct CliTerminal<'a, W: Write, I: LineInput, F> {
    output: &'a mut W,
    input: I,
    effects: F,
    state: PresentationState,
    offered: OfferedInputs,
    prompt: Option<LinePrompt>,
    feedback: TerminalFeedback,
    line: Vec<u8>,
    queued: VecDeque<u8>,
    ended: bool,
    skip_lf: bool,
    escape: u8,
}

impl<'a, W: Write, I: LineInput, F> CliTerminal<'a, W, I, F>
where
    F: FnMut(&[Command], &mut TerminalFeedback) -> io::Result<()>,
{
    pub fn new(output: &'a mut W, input: I, effects: F) -> Self {
        Self {
            output,
            input,
            effects,
            state: PresentationState::default(),
            offered: OfferedInputs::default(),
            prompt: None,
            feedback: TerminalFeedback::default(),
            line: Vec::new(),
            queued: VecDeque::new(),
            ended: false,
            skip_lf: false,
            escape: 0,
        }
    }

    fn draw(&mut self) -> io::Result<()> {
        writeln!(self.output, "{}", render_to_string(&self.state))?;
        if let Some(status) = &self.feedback.status {
            writeln!(self.output, "{status}")?;
        }
        if let Some((title, message)) = &self.feedback.alert {
            writeln!(self.output, "{title}\n{message}\n  Enter > ")?;
        } else if let Some(prompt) = &self.prompt {
            write!(self.output, "{}", prompt.prompt())?;
            if self.input.is_terminal() && !prompt.is_secret() {
                self.output.write_all(&self.line)?;
            }
        }
        self.output.flush()
    }

    fn submit(&mut self) -> io::Result<ControlFlow<(), Vec<Event>>> {
        writeln!(self.output)?;
        if self.feedback.alert.take().is_some() {
            self.line.clear();
            self.draw()?;
            return Ok(ControlFlow::Continue(vec![]));
        }
        let bytes = std::mem::take(&mut self.line);
        let line = String::from_utf8(bytes).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "terminal input is not UTF-8")
        })?;
        let prompt = self
            .prompt
            .as_mut()
            .ok_or_else(|| io::Error::other("no input prompt"))?;
        if let Some(event) = prompt.submit_line(&self.state, &line)? {
            self.prompt = None;
            return Ok(ControlFlow::Continue(vec![event]));
        }
        write!(self.output, "{}", prompt.prompt())?;
        self.output.flush()?;
        Ok(ControlFlow::Continue(vec![]))
    }

    fn echo(&mut self, bytes: &[u8]) -> io::Result<()> {
        if self.input.is_terminal() && !self.prompt.as_ref().is_some_and(LinePrompt::is_secret) {
            self.output.write_all(bytes)?;
            self.output.flush()?;
        }
        Ok(())
    }
}

impl<W: Write, I: LineInput, F> TerminalIo for CliTerminal<'_, W, I, F>
where
    F: FnMut(&[Command], &mut TerminalFeedback) -> io::Result<()>,
{
    fn render(&mut self, commands: &[Command]) -> io::Result<ControlFlow<()>> {
        let effects = self.state.apply(commands);
        (self.effects)(&effects, &mut self.feedback)?;
        if self.state.native_back_requested() {
            return Ok(ControlFlow::Break(()));
        }
        if !self
            .prompt
            .as_ref()
            .is_some_and(|prompt| prompt.is_current(&self.state))
        {
            self.line.clear();
            self.escape = 0;
            self.prompt = Some(LinePrompt::new(&self.state, &mut self.offered)?);
        }
        self.draw()?;
        Ok(ControlFlow::Continue(()))
    }

    fn wait_input(&mut self, timeout: Duration) -> io::Result<ControlFlow<(), Vec<Event>>> {
        if self.prompt.is_none() && !self.ended {
            let prompt = LinePrompt::new(&self.state, &mut self.offered)?;
            write!(self.output, "{}", prompt.prompt())?;
            self.output.flush()?;
            self.prompt = Some(prompt);
        }
        if self.queued.is_empty() && !self.ended {
            match self.input.read(timeout)? {
                InputChunk::Idle => return Ok(ControlFlow::Continue(vec![])),
                InputChunk::End => self.ended = true,
                InputChunk::Data(bytes) => self.queued.extend(bytes),
            }
        }
        while let Some(byte) = self.queued.pop_front() {
            if byte == 3 {
                return Ok(ControlFlow::Break(()));
            }
            if self.skip_lf {
                self.skip_lf = false;
                if byte == b'\n' {
                    continue;
                }
            }
            if self.escape != 0 {
                if self.escape == 1 && matches!(byte, b'[' | b'O') {
                    self.escape = 2;
                } else if self.escape == 1 || (0x40..=0x7e).contains(&byte) {
                    self.escape = 0;
                }
                continue;
            }
            match byte {
                27 => self.escape = 1,
                b'\n' | b'\r' => {
                    self.skip_lf = byte == b'\r';
                    return self.submit();
                }
                4 if self.line.is_empty() => return Ok(ControlFlow::Break(())),
                4 => return self.submit(),
                8 | 127 => {
                    if let Some(mut removed) = self.line.pop() {
                        while removed & 0xc0 == 0x80 {
                            let Some(previous) = self.line.pop() else {
                                break;
                            };
                            removed = previous;
                        }
                        self.echo(b"\x08 \x08")?;
                    }
                }
                b'\t' | 32..=255 => {
                    if self.line.len() == 65536 {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "terminal input line exceeds 64 KiB",
                        ));
                    }
                    self.line.push(byte);
                    self.echo(&[byte])?;
                }
                _ => {}
            }
        }
        if self.ended {
            if self.line.is_empty() {
                return Ok(ControlFlow::Break(()));
            }
            return self.submit();
        }
        Ok(ControlFlow::Continue(vec![]))
    }

    fn restore(&mut self) -> io::Result<()> {
        self.input.restore()
    }
}
