// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later
//! gvauchi: Core, linux-hw and the GTK display shell (ADR-078 step 4d.4):
//! this binary builds the engine and the hardware; the shell renders.
fn main() {
    eprintln!(
        "[linux-app] gvauchi with Core {}",
        vauchi_linux_app::wiring::core_version()
    );
    if let Some(resource_dir) = std::env::var_os("VAUCHI_LOCALES_DIR") {
        let _ = vauchi_app::i18n::init(std::path::Path::new(&resource_dir));
    }
    gtk_display::shell::run(
        Box::new(vauchi_linux_app::wiring::start_engine),
        Box::new(vauchi_linux_app::wiring::start_hardware),
    );
}
