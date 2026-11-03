// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later
//! gvauchi: Core, linux-hw and the GTK display shell (ADR-078 step 3b).
fn main() {
    eprintln!(
        "[linux-app] gvauchi with Core {}",
        vauchi_linux_app::wiring::core_version()
    );
    gtk_display::app::run_with(Box::new(vauchi_linux_app::wiring::start_hardware));
}
