#![warn(rust_2018_idioms, clippy::all, clippy::pedantic, clippy::nursery)]
#![warn(clippy::get_unwrap)]
#![cfg_attr(feature = "benchmark", feature(test))]

#[cfg(all(feature = "benchmark", test))]
extern crate test;

#[macro_use]
extern crate clap;
#[macro_use]
extern crate serde_derive;

pub mod skk;

#[macro_export]
macro_rules! const_panic {
    () => {
        let panic: [i32; 0] = [];
        #[allow(unconditional_panic)]
        panic[0];
    };
    ($msg: expr) => {
        const_panic!();
    };
}

#[macro_export]
macro_rules! const_assert {
    ($cond: expr) => {
        if !$cond {
            const_panic!();
        }
    };
}

#[macro_export]
macro_rules! const_assert_eq {
    ($left: expr, $right: expr) => {
        const_assert!($left == $right);
    };
}
