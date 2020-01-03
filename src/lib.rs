#![warn(rust_2018_idioms)]
#![warn(clippy::redundant_clone)]

#![cfg_attr(feature = "benchmark", feature(test))]

#[cfg(all(feature = "benchmark", test))]
extern crate test;

#[macro_use]
extern crate clap;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate serde_derive;

pub mod skk;
