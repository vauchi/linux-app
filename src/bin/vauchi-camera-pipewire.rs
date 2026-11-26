// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

fn main() {
    vauchi_linux_hw::camera_helper::run(
        std::sync::Arc::new(vauchi_linux_hw::PipeWireCamera::default()),
        vauchi_linux_hw::PipeWireCamera::available,
    )
}
