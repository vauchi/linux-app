// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later
//! The Linux app (ADR-078): assembles Core, the hardware shell `linux-hw`
//! and a display shell into one binary per pairing. All of its wiring
//! lives in [`wiring`]; nothing here decides behaviour (D-13).
pub mod wiring;
