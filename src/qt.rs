// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Qt main-loop wiring; Core and linux-hw remain owned by the app.
use crate::{engine::CommandEngine, wiring::HardwareBridge};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QtHostError {
    InvalidEvent,
    Hardware,
    Encoding,
    Display,
}

impl std::fmt::Display for QtHostError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidEvent => "Qt supplied an invalid event",
            Self::Hardware => "Linux hardware dispatch failed",
            Self::Encoding => "Could not encode the Qt command batch",
            Self::Display => "Qt rejected the command batch",
        })
    }
}
impl std::error::Error for QtHostError {}

pub struct QtHost<E: CommandEngine> {
    bridge: HardwareBridge<E>,
}

impl<E: CommandEngine> QtHost<E> {
    pub fn new(bridge: HardwareBridge<E>) -> Self {
        Self { bridge }
    }

    pub fn on_event(&mut self, json: &str) -> Result<(), QtHostError> {
        let event =
            vauchi_contract::event_from_json(json).map_err(|_| QtHostError::InvalidEvent)?;
        self.bridge
            .dispatch(event)
            .map_err(|_| QtHostError::Hardware)
    }

    pub fn poll(&mut self) -> Result<Option<String>, QtHostError> {
        self.bridge.poll().map_err(|_| QtHostError::Hardware)?;
        let commands = self.bridge.take_commands();
        if commands.is_empty() {
            return Ok(None);
        }
        serde_json::to_string(&commands)
            .map(Some)
            .map_err(|_| QtHostError::Encoding)
    }
}

/// Own the bridge until Qt exits; neither callback crosses a thread boundary.
pub fn run<E: CommandEngine + 'static>(
    args: &[String],
    bridge: HardwareBridge<E>,
) -> Result<i32, QtHostError> {
    let host = Rc::new(RefCell::new(QtHost::new(bridge)));
    let failure = Rc::new(Cell::new(None));
    let events = host.clone();
    let event_failure = failure.clone();
    let ticks = host.clone();
    let tick_failure = failure.clone();
    let exit = qt_display::run_with_poll(
        args,
        Box::new(move |json| {
            if event_failure.get().is_some() {
                return;
            }
            if let Err(error) = events.borrow_mut().on_event(json) {
                event_failure.set(Some(error));
                qt_display::stop();
            }
        }),
        Box::new(move || {
            if tick_failure.get().is_some() {
                return;
            }
            let batch = ticks.borrow_mut().poll();
            let result = batch.and_then(|batch| {
                batch.map_or(Ok(()), |json| {
                    qt_display::render(&json).map_err(|_| QtHostError::Display)
                })
            });
            if let Err(error) = result {
                tick_failure.set(Some(error));
                qt_display::stop();
            }
        }),
    );
    failure.get().map_or(Ok(exit), Err)
}
