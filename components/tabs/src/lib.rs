/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#![allow(unknown_lints)]
#![warn(rust_2018_idioms)]

#[macro_use]
pub mod error;
mod ffi;
mod record;
mod store;

// <TODO-ST>

pub use crate::store::TabsStore;
pub use error::{Error, ErrorKind, Result};
