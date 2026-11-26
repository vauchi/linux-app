// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

#[cfg(any(feature = "gtk", feature = "qt"))]
use vauchi_linux_hw::preview_source::PreviewDirectory;

#[cfg(feature = "gtk")]
pub(crate) fn install_gtk(directory: Option<PreviewDirectory>) {
    use gtk_display::core_ui::camera_preview::{PreviewSource, set_source_resolver};
    use std::{cell::RefCell, rc::Rc};
    let resolver = directory
        .and_then(|directory| directory.resolver().ok())
        .map(|resolver| {
            let resolver = RefCell::new(resolver);
            Rc::new(move |id: &str| {
                resolver
                    .borrow_mut()
                    .poll(id)
                    .map(|source| {
                        source.map(|source| PreviewSource {
                            node_id: source.node_id,
                            serial: source.serial,
                            remote_fd: source.remote_fd,
                        })
                    })
                    .map_err(|error| error.to_string())
            }) as gtk_display::core_ui::camera_preview::SourceResolver
        });
    set_source_resolver(resolver);
}

#[cfg(feature = "qt")]
pub(crate) fn qt_resolver(directory: Option<PreviewDirectory>) -> qt_display::SourceResolver {
    let mut resolver = directory.and_then(|directory| directory.resolver().ok());
    Box::new(move |id| {
        let Some(resolver) = resolver.as_mut() else {
            return Ok(None);
        };
        resolver
            .poll(id)
            .map(|source| {
                source.map(|source| qt_display::PreviewSource {
                    node_id: source.node_id,
                    serial: source.serial,
                    remote_fd: source.remote_fd,
                })
            })
            .map_err(|error| error.to_string())
    })
}
