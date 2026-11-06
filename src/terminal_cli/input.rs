// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    termios::{OptionalActions, Termios, tcgetattr, tcsetattr},
};
use std::{io, os::fd::AsFd, time::Duration};

#[derive(Debug, PartialEq, Eq)]
pub enum InputChunk {
    Idle,
    Data(Vec<u8>),
    End,
}

pub trait LineInput {
    fn read(&mut self, timeout: Duration) -> io::Result<InputChunk>;
    fn is_terminal(&self) -> bool {
        false
    }
    fn restore(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Exclusive reader of a descriptor. Poll before each unbuffered read so a
/// partial pipe write never blocks waiting for a newline. Descriptor flags stay unchanged.
pub struct FdInput<F: AsFd> {
    fd: F,
    original: Option<Termios>,
}

impl<F: AsFd> FdInput<F> {
    pub fn new(fd: F) -> io::Result<Self> {
        let original = match tcgetattr(&fd) {
            Ok(attributes) => Some(attributes),
            Err(rustix::io::Errno::NOTTY) => None,
            Err(error) => return Err(error.into()),
        };
        let input = Self { fd, original };
        if let Some(original) = &input.original {
            let mut raw = original.clone();
            raw.make_raw();
            raw.output_modes = original.output_modes;
            tcsetattr(&input.fd, OptionalActions::Now, &raw)?;
        }
        Ok(input)
    }
}

impl<F: AsFd> LineInput for FdInput<F> {
    fn read(&mut self, timeout: Duration) -> io::Result<InputChunk> {
        let timeout = Timespec::try_from(timeout).map_err(io::Error::other)?;
        let mut descriptors = [PollFd::new(&self.fd, PollFlags::IN)];
        match poll(&mut descriptors, Some(&timeout)) {
            Ok(0) | Err(rustix::io::Errno::INTR) => return Ok(InputChunk::Idle),
            Err(error) => return Err(error.into()),
            Ok(_) => {}
        }
        let mut bytes = [0; 4096];
        match rustix::io::read(&self.fd, &mut bytes) {
            Ok(0) => Ok(InputChunk::End),
            Ok(count) => Ok(InputChunk::Data(bytes[..count].to_vec())),
            Err(rustix::io::Errno::INTR | rustix::io::Errno::AGAIN) => Ok(InputChunk::Idle),
            Err(error) => Err(error.into()),
        }
    }

    fn is_terminal(&self) -> bool {
        self.original.is_some()
    }

    fn restore(&mut self) -> io::Result<()> {
        if let Some(original) = &self.original {
            tcsetattr(&self.fd, OptionalActions::Now, original)?;
            self.original = None;
        }
        Ok(())
    }
}

impl<F: AsFd> Drop for FdInput<F> {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}
