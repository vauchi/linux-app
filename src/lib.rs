// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later
//! The Linux app (ADR-078): assembles Core, the hardware shell `linux-hw`
//! and a display shell into one binary per pairing. Its wiring lives in
//! [`wiring`], with the engine newtype, the locale report and the storage
//! set-up beside it; nothing here decides behaviour (D-13).
pub mod engine;
pub mod locale;
#[cfg(feature = "qt")]
pub mod qt;
pub mod relay_setting;
pub mod storage;
pub mod terminal;
#[cfg(feature = "cli")]
pub mod terminal_cli;
#[cfg(any(feature = "cli", feature = "tui"))]
mod terminal_signals;
pub mod wiring;

#[cfg(feature = "cli")]
pub use ::cli_display;
#[cfg(feature = "tui")]
pub use ::tui_display;
