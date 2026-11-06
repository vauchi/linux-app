// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args_os().nth(1).is_some() {
        return Err(
            "Usage: vauchi (interactive mode; command arguments are not supported yet)".into(),
        );
    }
    vauchi_linux_app::wiring::run_cli()
}
