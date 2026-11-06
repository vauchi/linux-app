// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

//! TUI display adapter with bounded native event polling.

use crossterm::event::{self, Event as NativeEvent, KeyCode, KeyEventKind, KeyModifiers};
use ratatui_core::{backend::Backend, terminal::Terminal};
use std::{io, ops::ControlFlow, time::Duration};
use vauchi_contract::{Command, Event};
use vauchi_tui_display::{SessionView, TerminalSession, ui::presentation_input::KeyOutcome};

use crate::terminal::TerminalIo;

pub struct TerminalMode;

impl TerminalMode {
    pub fn enter() -> io::Result<Self> {
        crossterm::terminal::enable_raw_mode()?;
        let mode = Self;
        crossterm::execute!(io::stdout(), crossterm::terminal::EnterAlternateScreen)?;
        Ok(mode)
    }
}

impl Drop for TerminalMode {
    fn drop(&mut self) {
        let _ = crossterm::execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen);
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

pub trait TerminalEvents {
    fn next(&mut self, timeout: Duration) -> io::Result<Option<NativeEvent>>;
}

pub struct CrosstermEvents;
impl TerminalEvents for CrosstermEvents {
    fn next(&mut self, timeout: Duration) -> io::Result<Option<NativeEvent>> {
        if event::poll(timeout)? {
            event::read().map(Some)
        } else {
            Ok(None)
        }
    }
}

#[derive(Default)]
pub struct TerminalFeedback {
    pub status: Option<String>,
    pub alert: Option<(String, String)>,
}

pub struct TuiTerminal<'a, B: Backend, I, F> {
    output: &'a mut Terminal<B>,
    input: I,
    effects: F,
    session: TerminalSession,
    feedback: TerminalFeedback,
}

impl<'a, B: Backend, I: TerminalEvents, F> TuiTerminal<'a, B, I, F>
where
    F: FnMut(&[Command], &mut TerminalFeedback) -> io::Result<()>,
{
    pub fn new(output: &'a mut Terminal<B>, input: I, effects: F) -> Self {
        Self {
            output,
            input,
            effects,
            session: TerminalSession::default(),
            feedback: TerminalFeedback::default(),
        }
    }
}

impl<B: Backend, I, F> TuiTerminal<'_, B, I, F>
where
    B::Error: std::error::Error + Send + Sync + 'static,
{
    fn draw(&mut self) -> io::Result<()> {
        self.output
            .draw(|frame| {
                self.session.draw(
                    frame,
                    frame.area(),
                    SessionView {
                        status: self.feedback.status.as_deref(),
                        alert: self.feedback.alert.as_ref(),
                        ..SessionView::default()
                    },
                );
            })
            .map_err(io::Error::other)?;
        Ok(())
    }
}

impl<B: Backend, I: TerminalEvents, F> TerminalIo for TuiTerminal<'_, B, I, F>
where
    B::Error: std::error::Error + Send + Sync + 'static,
    F: FnMut(&[Command], &mut TerminalFeedback) -> io::Result<()>,
{
    fn render(&mut self, commands: &[Command]) -> io::Result<ControlFlow<()>> {
        let effects = self.session.apply_commands(commands);
        (self.effects)(&effects, &mut self.feedback)?;
        if self.session.presentation().native_back_requested() {
            return Ok(ControlFlow::Break(()));
        }
        self.draw()?;
        Ok(ControlFlow::Continue(()))
    }

    fn wait_input(&mut self, timeout: Duration) -> io::Result<ControlFlow<(), Vec<Event>>> {
        let Some(event) = self.input.next(timeout)? else {
            return Ok(ControlFlow::Continue(vec![]));
        };
        match event {
            NativeEvent::Key(key) if key.kind != KeyEventKind::Release => {
                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    return Ok(ControlFlow::Break(()));
                }
                if self.feedback.alert.is_some() {
                    if matches!(key.code, KeyCode::Esc | KeyCode::Enter) {
                        self.feedback.alert = None;
                        self.draw()?;
                    }
                    return Ok(ControlFlow::Continue(vec![]));
                }
                match self.session.handle_key(key) {
                    KeyOutcome::Events(events) => return Ok(ControlFlow::Continue(events)),
                    KeyOutcome::Quit => return Ok(ControlFlow::Break(())),
                    KeyOutcome::Consumed => self.draw()?,
                }
            }
            NativeEvent::Resize(..) => self.draw()?,
            _ => {}
        }
        Ok(ControlFlow::Continue(vec![]))
    }
}
