// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared Core adapter with an optional GTK EngineLink implementation.
use std::cell::RefCell;
use std::sync::mpsc::Receiver;

use vauchi_app::ui::AppEngine;
use vauchi_contract::{Command, Event};

/// Core's command/event boundary, supplied to the app's hardware bridge.
pub trait CommandEngine {
    fn dispatch(&self, event: Event) -> Vec<Command>;
    fn initial_commands(&self) -> Vec<Command>;
    fn take_invalidation(&self) -> bool {
        false
    }
}

pub struct CoreEngine {
    engine: RefCell<AppEngine>,
    invalidations: Receiver<()>,
}

impl CoreEngine {
    pub fn new(engine: AppEngine) -> Self {
        let invalidations = engine.subscribe_presentation_invalidations();
        Self {
            engine: RefCell::new(engine),
            invalidations,
        }
    }

    pub fn dispatch(&self, event: Event) -> Vec<Command> {
        let outcome = self.engine.borrow_mut().dispatch(event);
        outcome.unwrap_or_else(|error| {
            // The kind only: a rejection's Display text can echo user input.
            eprintln!("[CoreUI] Failed: dispatch rejected: {}", error.kind());
            self.engine.borrow().reject_dispatch(&error)
        })
    }

    pub fn initial_commands(&self) -> Vec<Command> {
        self.engine
            .borrow_mut()
            .initial_commands()
            .unwrap_or_default()
    }

    pub fn take_invalidation(&self) -> bool {
        self.invalidations.try_recv().is_ok()
    }
}

#[cfg(feature = "gtk")]
impl gtk_display::core_ui::engine_link::EngineLink for CoreEngine {
    fn dispatch(&self, event: Event) -> Vec<Command> {
        self.dispatch(event)
    }

    fn initial_commands(&self) -> Vec<Command> {
        self.initial_commands()
    }

    fn take_invalidation(&self) -> bool {
        self.take_invalidation()
    }
}

impl CommandEngine for CoreEngine {
    fn dispatch(&self, event: Event) -> Vec<Command> {
        CoreEngine::dispatch(self, event)
    }

    fn initial_commands(&self) -> Vec<Command> {
        CoreEngine::initial_commands(self)
    }

    fn take_invalidation(&self) -> bool {
        CoreEngine::take_invalidation(self)
    }
}
