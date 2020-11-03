#![warn(rust_2018_idioms, clippy::all, clippy::pedantic, clippy::nursery)]
#![warn(clippy::get_unwrap)]

use regex::Regex;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

/// 下記のような行を取得し、 `total_map` に NAME を key として N を `Vec` で追加する
///
/// `"NAME total rps.=N"`
/// `"NAME total ms.=N"`
fn run_cargo(test_name: &str, total_map: &mut BTreeMap<String, Vec<usize>>) {
    let re_name_unit = Regex::new(r"^([^ ]+) +.+(?:rps|ms)\.=(\d+)").unwrap();
    let cargo_args = if test_name.is_empty() {
        vec!["test", "--release", "--", "--nocapture", "--test-threads=1"]
    } else {
        vec![
            "test",
            "--release",
            test_name,
            "--",
            "--nocapture",
            "--test-threads=1",
        ]
    };
    let mut process = Command::new("cargo")
        .args(&cargo_args)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    for line in BufReader::new(process.stdout.take().unwrap()).lines() {
        let line = line.unwrap();
        if line.contains(" total ") {
            if let Some(m) = re_name_unit.captures(&line) {
                let name = &m[1];
                let unit = m[2].parse::<usize>().unwrap_or(0);
                total_map
                    .entry(String::from(name))
                    .or_insert_with(Vec::new)
                    .push(unit);
            }
        }
        println!("{}", line);
    }
    process.wait().unwrap();
}

fn print_total_map(total_map: &BTreeMap<String, Vec<usize>>) {
    const IS_SHOW_CHOMPED_VEC_LENGTH_LIMIT: usize = 5;
    for key in total_map.keys() {
        if total_map[key].is_empty() {
            panic!("illegal vec");
        } else {
            let mut vec = total_map[key].clone();
            vec.sort_unstable();
            let vec = vec;
            let is_show_chomped_vec = vec.len() >= IS_SHOW_CHOMPED_VEC_LENGTH_LIMIT;
            let chomped_vec = if is_show_chomped_vec {
                vec.iter().skip(1).take(vec.len() - 2).copied().collect()
            } else {
                Vec::new()
            };
            println!("{}", key);
            println!("        vec={:?}", vec);
            println!("         median {}", vec[vec.len() / 2]);
            println!("        average {}", vec.iter().sum::<usize>() / vec.len());
            if is_show_chomped_vec {
                println!("chomped_vec={:?}", chomped_vec);
                println!(
                    "chomped average {}",
                    chomped_vec.iter().sum::<usize>() / chomped_vec.len()
                );
            }
        }
    }
}

fn main() {
    const DEFAULT_LOOP_COUNT: usize = 5;
    let mut total_map = BTreeMap::new();
    let mut loop_count = DEFAULT_LOOP_COUNT;
    let mut test_name = String::new();
    match std::env::args().len() {
        2 => {
            if let Ok(count) = std::env::args().nth(1).unwrap().parse::<usize>() {
                loop_count = count;
            }
        }
        3 => {
            if let Ok(count) = std::env::args().nth(1).unwrap().parse::<usize>() {
                loop_count = count;
            }
            test_name = std::env::args().nth(2).unwrap();
            println!("test_name={}", test_name);
        }
        1 => {}
        _ => {
            panic!("usage: cargo run --release --bin=test_wrapper -- loop_count [test_name]");
        }
    }
    for i in 1..=loop_count {
        println!("loop = {:>3}/{:>3}", i, loop_count);
        run_cargo(&test_name, &mut total_map);
        print_total_map(&total_map);
    }
    println!("================");
}
