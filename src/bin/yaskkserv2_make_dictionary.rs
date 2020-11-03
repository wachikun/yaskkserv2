#![warn(rust_2018_idioms, clippy::all, clippy::pedantic, clippy::nursery)]
#![warn(clippy::get_unwrap)]

fn main() {
    if let Err(e) = yaskkserv2::skk::run_yaskkserv2_make_dictionary() {
        println!("Error: {}", e);
    }
}
