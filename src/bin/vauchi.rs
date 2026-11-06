// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

use clap::Command as CliArguments;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    CliArguments::new("vauchi")
        .about("Interactive Vauchi client")
        .version(env!("CARGO_PKG_VERSION"))
        .get_matches();
    vauchi_linux_app::wiring::run_cli()
}
