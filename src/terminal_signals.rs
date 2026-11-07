// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

use signal_hook::{
    SigId,
    consts::{SIGHUP, SIGINT, SIGQUIT, SIGTERM},
    flag, low_level,
};
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

/// Install only in a dedicated terminal process, before entering native input mode.
/// Unregistering signal-hook actions does not restore the default OS handler.
pub(crate) struct SignalShutdown {
    requested: Arc<AtomicBool>,
    registrations: Vec<SigId>,
}

impl SignalShutdown {
    pub(crate) fn new() -> io::Result<Self> {
        let mut shutdown = Self {
            requested: Arc::new(AtomicBool::new(false)),
            registrations: Vec::new(),
        };
        for signal in [SIGINT, SIGTERM, SIGHUP, SIGQUIT] {
            shutdown
                .registrations
                .push(flag::register(signal, shutdown.requested.clone())?);
        }
        Ok(shutdown)
    }

    pub(crate) fn requested(&self) -> bool {
        self.requested.load(Ordering::SeqCst)
    }
}

impl Drop for SignalShutdown {
    fn drop(&mut self) {
        for registration in self.registrations.drain(..) {
            low_level::unregister(registration);
        }
    }
}
