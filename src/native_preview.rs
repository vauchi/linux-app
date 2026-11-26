// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

#[cfg(any(feature = "gtk", feature = "qt"))]
use vauchi_linux_hw::frame_directory::{FrameDirectory, FrameResource};

#[cfg(feature = "gtk")]
pub(crate) fn install_gtk(directory: Option<FrameDirectory>) {
    use gtk_display::core_ui::camera_preview::{self, FrameResource as DisplayResource};
    camera_preview::set_frame_resolver(Some(std::rc::Rc::new(move |id, previous| {
        let Some(directory) = &directory else {
            return Ok(DisplayResource::Absent);
        };
        directory
            .poll(id, previous)
            .map(|resource| match resource {
                FrameResource::Absent => DisplayResource::Absent,
                FrameResource::Unchanged => DisplayResource::Unchanged,
                FrameResource::New { generation, fd } => DisplayResource::New { generation, fd },
            })
            .map_err(|error| error.to_string())
    })));
}

#[cfg(feature = "qt")]
pub(crate) fn qt_resolver(directory: Option<FrameDirectory>) -> qt_display::FrameResolver {
    Box::new(move |id, previous| {
        let Some(directory) = &directory else {
            return Ok(qt_display::FrameResource::Absent);
        };
        directory
            .poll(id, previous)
            .map(|resource| match resource {
                FrameResource::Absent => qt_display::FrameResource::Absent,
                FrameResource::Unchanged => qt_display::FrameResource::Unchanged,
                FrameResource::New { generation, fd } => {
                    qt_display::FrameResource::New { generation, fd }
                }
            })
            .map_err(|error| error.to_string())
    })
}
