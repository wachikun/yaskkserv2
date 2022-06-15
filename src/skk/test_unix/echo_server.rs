//! echo server
//!
//! echo server は Rust 内蔵版と C 版がある。 C 版は gcc でビルドし外部コマンドとして test
//! する。ビルドに失敗したり echo server バイナリを起動できなかった場合は何もせず成功する。

use mio::net::{TcpListener, TcpStream};
use mio::{Events, Poll, Token};
use std::io::Read;

use crate::skk::test_unix::{
    get_take_count, setup, wait_server, BufReader, ConnectSendCompare,
    ConnectSendCompareRunParameter, Path, Protocol, TcpStreamSkk, Write, MANY_THREADS,
};
use crate::skk::Encoding;

/// src/skk/yaskkserv2/mod.rs に同じ struct があることに注意。
struct MioSocket {
    buffer_stream: BufReader<TcpStream>,
}

impl MioSocket {
    fn new(stream: TcpStream) -> Self {
        Self {
            buffer_stream: BufReader::new(stream),
        }
    }
}

/// empty な index を取得する
///
/// src/skk/yaskkserv2/mod.rs に同じ関数があることに注意。
///
/// # Panics
/// index が見付からない場合、 panic!() することに注意。
fn get_empty_sockets_index(
    sockets: &[Option<MioSocket>],
    sockets_length: usize,
    next_socket_index: usize,
) -> usize {
    let mut index = next_socket_index;
    index += 1;
    if index >= sockets_length {
        index = 0;
    }
    if sockets[index].is_none() {
        return index;
    }
    for _ in 0..sockets_length {
        index += 1;
        if index >= sockets_length {
            index = 0;
        }
        if sockets[index].is_none() {
            return index;
        }
    }
    panic!("illegal sockets slice");
}

fn connect_for_echo_server(name: &str, port: &'static str, is_sequential: bool, threads: usize) {
    let jisyo_full_path = &Path::get_full_path_yaskkserv2_jisyo(Encoding::Utf8);
    wait_server(port);
    let parameter =
        ConnectSendCompareRunParameter::new(jisyo_full_path, name, port, Protocol::Echo, false)
            .encoding(Encoding::Utf8)
            .is_sequential(is_sequential)
            .threads(threads)
            .is_send_lf(true);
    ConnectSendCompare::run(parameter);
}

fn echo_server_std_net_tcp_raw_server(
    port: &'static str,
    take_count: usize,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name(String::from(std::thread::current().name().unwrap()))
        .spawn(move || {
            let listener = match std::net::TcpListener::bind(format!("localhost:{}", port)) {
                Ok(ok) => ok,
                Err(e) => {
                    println!("bind failed  error={:?}", e);
                    return;
                }
            };
            let mut thread_handles = Vec::new();
            for stream in listener.incoming().take(take_count) {
                match stream {
                    Ok(mut stream) => {
                        thread_handles.push(
                            std::thread::Builder::new()
                                .name(String::from(std::thread::current().name().unwrap()))
                                .spawn(move || {
                                    let mut buffer = vec![0; 32 * 1024];
                                    while match stream.read(&mut buffer) {
                                        Ok(0) => false,
                                        Ok(size) => {
                                            stream.write_all_flush(&buffer[..size]).unwrap();
                                            // FIXME! 厳密にはこの判定は正しくないが test echo server なので許容する
                                            size != 1 || buffer[0] != b'0'
                                        }
                                        Err(e) => panic!("{:#?}", e),
                                    } {}
                                })
                                .unwrap(),
                        );
                    }
                    Err(e) => panic!("{:#?}", e),
                }
            }
            for handle in thread_handles {
                let _ignore_error_and_continue = handle.join();
            }
            drop(listener);
        })
        .unwrap()
}

fn echo_server_mio_raw_server(
    port: &'static str,
    max_connections: usize,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name(String::from(std::thread::current().name().unwrap()))
        .spawn(move || {
            const LISTENER: Token = Token(1024);
            let mut sockets: Vec<Option<MioSocket>> = Vec::new();
            for _ in 0..max_connections {
                sockets.push(None);
            }
            let sockets_length = sockets.len();
            let mut sockets_some_count = 0;
            let mut next_socket_index = 0;
            let mut poll = Poll::new().unwrap();
            let mut listener =
                TcpListener::bind(format!("127.0.0.1:{}", port).parse().unwrap()).unwrap();
            poll.registry()
                .register(&mut listener, LISTENER, mio::Interest::READABLE)
                .unwrap();
            let mut events = Events::with_capacity(1024);
            let mut buffer = vec![0; 32 * 1024];
            'outer: loop {
                poll.poll(&mut events, None).unwrap();
                for event in &events {
                    match event.token() {
                        LISTENER => loop {
                            match listener.accept() {
                                Ok((mut socket, _)) => {
                                    let token = Token(next_socket_index);
                                    poll.registry()
                                        .register(&mut socket, token, mio::Interest::READABLE)
                                        .unwrap();
                                    sockets[usize::from(token)] = Some(MioSocket::new(socket));
                                    sockets_some_count += 1;
                                    if sockets_some_count < max_connections {
                                        next_socket_index = get_empty_sockets_index(
                                            &sockets,
                                            sockets_length,
                                            next_socket_index,
                                        );
                                    }
                                }
                                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                    break;
                                }
                                Err(e) => panic!("e={:?}", e),
                            }
                        },
                        token => {
                            let socket = match &mut sockets[usize::from(token)] {
                                Some(socket) => socket,
                                None => panic!("sockets get failed"),
                            };
                            let mut is_exit = false;
                            while match socket.buffer_stream.read(&mut buffer) {
                                Ok(0) => {
                                    is_exit = true;
                                    false
                                }
                                Ok(size) => {
                                    socket
                                        .buffer_stream
                                        .get_mut()
                                        .write_all(&buffer[..size])
                                        .unwrap();
                                    socket.buffer_stream.get_mut().flush().unwrap();
                                    // FIXME! 厳密にはこの判定は正しくないが test echo server なので許容する
                                    if size == 1 && buffer[0] == b'0' {
                                        break 'outer;
                                    }
                                    true
                                }
                                Err(e) => {
                                    if e.kind() == std::io::ErrorKind::WouldBlock {
                                        false
                                    } else {
                                        panic!("read panic {:#?}", e);
                                    }
                                }
                            } {}
                            if is_exit {
                                poll.registry()
                                    .deregister(socket.buffer_stream.get_mut())
                                    .unwrap();
                                sockets[usize::from(token)] = None;
                                sockets_some_count -= 1;
                                next_socket_index = usize::from(token);
                            }
                        }
                    }
                }
            }
        })
        .unwrap()
}

fn echo_server_c_server(
    name: &'static str,
    port: &'static str,
    is_sequential: bool,
    threads: usize,
) {
    let child = match std::process::Command::new(Path::get_full_path_echo_server())
        .arg(port)
        .arg(format!("{}", get_take_count(threads)))
        .spawn()
    {
        Ok(ok) => ok,
        Err(e) => {
            println!("Error(test success): echo_server  error={:?}", e);
            return;
        }
    };
    connect_for_echo_server(name, port, is_sequential, threads);
    // FIXME! この kill だけでは処理できないケースが多々ある
    let _droppable = std::process::Command::new("kill")
        .arg("-TERM")
        .arg(format!("{}", child.id()))
        .spawn()
        .unwrap();
}

#[test]
fn echo_server_benchmark_std_net_tcp_raw_sequential_test() {
    let name = "echo_server_benchmark_std_net_tcp_raw_sequential";
    setup::setup_and_wait(name);
    let port = "10001";
    let threads = 1;
    let thread_handle = echo_server_std_net_tcp_raw_server(port, get_take_count(threads));
    connect_for_echo_server(name, port, true, threads);
    thread_handle.join().unwrap();
    setup::exit();
}

#[test]
fn echo_server_benchmark_std_net_tcp_raw_random_test() {
    let name = "echo_server_benchmark_std_net_tcp_raw_random";
    setup::setup_and_wait(name);
    let port = "10002";
    let threads = 1;
    let thread_handle = echo_server_std_net_tcp_raw_server(port, get_take_count(threads));
    connect_for_echo_server(name, port, false, threads);
    thread_handle.join().unwrap();
    setup::exit();
}

#[test]
fn echo_server_benchmark_std_net_tcp_raw_sequential_multithreads_test() {
    let name = "echo_server_benchmark_std_net_tcp_raw_sequential_multithreads";
    setup::setup_and_wait(name);
    let port = "10003";
    let threads = MANY_THREADS;
    let thread_handle = echo_server_std_net_tcp_raw_server(port, get_take_count(threads));
    connect_for_echo_server(name, port, true, threads);
    thread_handle.join().unwrap();
    setup::exit();
}

#[test]
fn echo_server_benchmark_std_net_tcp_raw_random_multithreads_test() {
    let name = "echo_server_benchmark_std_net_tcp_raw_random_multithreads";
    setup::setup_and_wait(name);
    let port = "10004";
    let threads = MANY_THREADS;
    let thread_handle = echo_server_std_net_tcp_raw_server(port, get_take_count(threads));
    connect_for_echo_server(name, port, false, threads);
    thread_handle.join().unwrap();
    setup::exit();
}

#[test]
fn echo_server_benchmark_mio_raw_sequential_test() {
    let name = "echo_server_benchmark_mio_raw_sequential";
    setup::setup_and_wait(name);
    let port = "10010";
    let threads = 1;
    let thread_handle = echo_server_mio_raw_server(port, threads);
    connect_for_echo_server(name, port, true, threads);
    thread_handle.join().unwrap();
    setup::exit();
}

#[test]
fn echo_server_benchmark_mio_raw_sequential_multithreads_test() {
    let name = "echo_server_benchmark_mio_raw_sequential_multithreads_test";
    setup::setup_and_wait(name);
    let port = "10011";
    let threads = MANY_THREADS;
    let thread_handle = echo_server_mio_raw_server(port, threads);
    connect_for_echo_server(name, port, true, threads);
    thread_handle.join().unwrap();
    setup::exit();
}

#[test]
fn echo_server_benchmark_c_sequential_test() {
    let name = "echo_server_benchmark_c_sequential";
    setup::setup_and_wait(name);
    let port = "10020";
    let threads = 1;
    echo_server_c_server(name, port, true, threads);
    setup::exit();
}

#[test]
fn echo_server_benchmark_c_random_test() {
    let name = "echo_server_benchmark_c_random";
    setup::setup_and_wait(name);
    let port = "10021";
    let threads = 1;
    echo_server_c_server(name, port, false, threads);
    setup::exit();
}
