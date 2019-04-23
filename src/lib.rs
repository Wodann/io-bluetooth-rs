#[macro_use]
extern crate cfg_if;

#[cfg(windows)]
extern crate winapi;

pub mod bt;

mod sys;
mod sys_common;
