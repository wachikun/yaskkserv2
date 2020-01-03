#![warn(rust_2018_idioms)]
#![warn(clippy::redundant_clone)]

fn main() {
    if let Err(e) = yaskkserv2::skk::run_yaskkserv2() {
        println!("Error: {}", e);
    }
}
