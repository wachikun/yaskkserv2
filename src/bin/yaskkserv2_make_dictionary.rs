#![warn(rust_2018_idioms)]
#![warn(clippy::redundant_clone)]

fn main() {
    if let Err(e) = yaskkserv2::skk::run_yaskkserv2_make_dictionary() {
        println!("Error: {}", e);
    }
}
