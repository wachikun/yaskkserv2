#[cfg(test)]
use crate::skk::yaskkserv2::server::test_unix::ServerDebug;
use crate::skk::yaskkserv2::{
    BufReader, Config, DictionaryFile, DictionaryReader, HandleClientResult, OnMemory, Server,
    SkkError, TcpStream, TcpStreamSkk, Yaskkserv2, PKG_VERSION, PROTOCOL_MAXIMUM_LENGTH,
    PROTOCOL_MINIMUM_LENGTH,
};
#[cfg(feature = "assert_paranoia")]
use crate::{const_assert, const_panic};

impl Server {
    pub(in crate::skk) fn new() -> Self {
        Self {
            config: Config::new(),
            dictionary: DictionaryReader::new(),
        }
    }

    pub(in crate::skk) fn setup(&mut self, config: Config, on_memory: OnMemory) {
        self.config = config.clone();
        self.dictionary.setup(config, on_memory);
    }

    pub(in crate::skk) fn handle_client(
        &self,
        buffer_stream: &mut BufReader<TcpStream>,
        dictionary_file: &mut DictionaryFile,
        buffer: &mut [u8],
    ) -> HandleClientResult {
        let stream = buffer_stream.get_mut();
        match buffer[0] {
            b'0' => return HandleClientResult::Exit,
            b'1' => {
                if self.config.is_midashi_utf8 {
                    let utf8_to_euc_buffer = crate::skk::encoding_simple::Euc::encode(buffer);
                    if let Ok(mut utf8_to_euc_buffer) = utf8_to_euc_buffer {
                        self.handle_client_protocol_1(
                            stream,
                            dictionary_file,
                            &mut utf8_to_euc_buffer,
                        );
                    } else {
                        Self::send_and_log_protocol_error(stream, "1", &SkkError::Encoding);
                    }
                } else {
                    self.handle_client_protocol_1(stream, dictionary_file, buffer);
                }
            }
            b'2' => stream.write_all_flush_ignore_error(format!("{PKG_VERSION} ").as_bytes()),
            b'3' => stream.write_all_flush_ignore_error(
                self.config
                    .hostname_and_ip_address_for_protocol_3
                    .as_bytes(),
            ),
            b'4' => {
                if self.config.is_midashi_utf8 {
                    let utf8_to_euc_buffer = crate::skk::encoding_simple::Euc::encode(buffer);
                    if let Ok(mut utf8_to_euc_buffer) = utf8_to_euc_buffer {
                        self.handle_client_protocol_4(
                            stream,
                            dictionary_file,
                            &mut utf8_to_euc_buffer,
                        );
                    } else {
                        Self::send_and_log_protocol_error(stream, "4", &SkkError::Encoding);
                    }
                } else {
                    self.handle_client_protocol_4(stream, dictionary_file, buffer);
                }
            }
            _ => {
                let _ignore_error = stream.write_error_flush();
            }
        }
        HandleClientResult::Continue
    }

    fn validate_buffer_for_protocol_1_and_4(buffer: &[u8]) -> bool {
        let buffer_len = buffer.len();
        if !(PROTOCOL_MINIMUM_LENGTH..=PROTOCOL_MAXIMUM_LENGTH).contains(&buffer_len) {
            return false;
        }
        #[cfg(feature = "assert_paranoia")]
        {
            const_assert!(buffer[buffer_len - 1] == b' ');
        }
        true
    }

    fn send_and_log_protocol_error(stream: &mut TcpStream, protocol: &str, e: &SkkError) {
        Yaskkserv2::log_error(&format!("protocol {protocol} error={e}"));
        let _ignore_error = stream.write_error_flush();
    }

    fn handle_client_protocol_1(
        &self,
        stream: &mut TcpStream,
        dictionary_file: &mut DictionaryFile,
        buffer: &mut [u8],
    ) {
        if !Self::validate_buffer_for_protocol_1_and_4(buffer) {
            let _ignore_error = stream.write_error_flush();
            return;
        }
        match self.dictionary.read_candidates(dictionary_file, buffer) {
            Ok(mut candidates) => {
                if Yaskkserv2::is_empty_candidates(&candidates) {
                    buffer[0] = b'4';
                    if let Some(last) = buffer.last() {
                        if *last == b'\n' || *last == b'\r' {
                            stream.write_all_flush_ignore_error(buffer);
                        } else {
                            let mut lf_appended_buffer = Vec::from(buffer);
                            lf_appended_buffer.push(b'\n');
                            stream.write_all_flush_ignore_error(&lf_appended_buffer);
                        }
                    } else {
                        Self::send_and_log_protocol_error(stream, "1", &SkkError::BrokenDictionary);
                    }
                } else {
                    candidates.push(b'\n');
                    #[cfg(not(test))]
                    stream.write_all_flush_ignore_error(&candidates);
                    #[cfg(test)]
                    {
                        if self.config.is_debug_send {
                            Self::send_bytes_debug(stream, &candidates);
                        } else {
                            stream.write_all_flush_ignore_error(&candidates);
                        }
                    }
                }
            }
            Err(e) => Self::send_and_log_protocol_error(stream, "1", &e),
        }
    }

    fn handle_client_protocol_4(
        &self,
        stream: &mut TcpStream,
        dictionary_file: &mut DictionaryFile,
        buffer: &mut [u8],
    ) {
        if !Self::validate_buffer_for_protocol_1_and_4(buffer) {
            let _ignore_error = stream.write_error_flush();
            return;
        }
        match self.dictionary.read_abbrev(dictionary_file, buffer) {
            Ok(mut candidates) => {
                if Yaskkserv2::is_empty_candidates(&candidates) {
                    if let Some(last) = buffer.last() {
                        if *last == b'\n' || *last == b'\r' {
                            stream.write_all_flush_ignore_error(buffer);
                        } else {
                            let mut lf_appended_buffer = Vec::from(buffer);
                            lf_appended_buffer.push(b'\n');
                            stream.write_all_flush_ignore_error(&lf_appended_buffer);
                        }
                    } else {
                        Self::send_and_log_protocol_error(stream, "4", &SkkError::BrokenDictionary);
                    }
                } else {
                    candidates.push(b'\n');
                    stream.write_all_flush_ignore_error(&candidates);
                }
            }
            Err(e) => Self::send_and_log_protocol_error(stream, "4", &e),
        }
    }
}

#[cfg(test)]
pub(in crate::skk) mod test_unix {
    use rand::Rng;

    use crate::skk::yaskkserv2::{
        DictionaryFile, Server, TcpStream, TcpStreamSkk, Write, Yaskkserv2,
    };

    struct ServerDebugImpl;

    impl ServerDebugImpl {
        #[allow(clippy::branches_sharing_code)]
        fn send_split(mut stream: &TcpStream, buffer: &[u8], split: usize) {
            if split < 2 {
                if let Err(e) = stream.write_all(buffer) {
                    Yaskkserv2::log_error(&format!("{e}"));
                }
                if let Err(e) = stream.flush() {
                    Yaskkserv2::log_error(&format!("{e}"));
                }
            } else {
                if let Err(e) =
                    stream.write_all(&buffer.iter().take(split).copied().collect::<Vec<u8>>())
                {
                    Yaskkserv2::log_error(&format!("{e}"));
                }
                if let Err(e) =
                    stream.write_all(&buffer.iter().skip(split).copied().collect::<Vec<u8>>())
                {
                    Yaskkserv2::log_error(&format!("{e}"));
                }
                if let Err(e) = stream.flush() {
                    Yaskkserv2::log_error(&format!("{e}"));
                }
            }
        }

        fn send_half_each(stream: &TcpStream, buffer: &[u8]) {
            Self::send_split(stream, buffer, buffer.len() / 2);
        }

        fn send_split_random(stream: &TcpStream, buffer: &[u8]) {
            Self::send_split(
                stream,
                buffer,
                rand::thread_rng().gen_range(1..buffer.len()),
            );
        }

        #[allow(dead_code)]
        fn send_double(mut stream: &TcpStream, buffer: &[u8]) {
            if let Err(e) =
                stream.write_all(&buffer.iter().chain(buffer).copied().collect::<Vec<u8>>())
            {
                Yaskkserv2::log_error(&format!("{e}"));
            }
            if let Err(e) = stream.flush() {
                Yaskkserv2::log_error(&format!("{e}"));
            }
        }
    }

    pub(in crate::skk) trait ServerDebug {
        fn send_bytes_debug_bad_condition(stream: &mut TcpStream, buffer: &[u8]);
        fn send_bytes_debug(stream: &mut TcpStream, buffer: &[u8]);
        fn send_bytes_std_net_tcp(stream: &std::net::TcpStream, buffer: &[u8]);
        fn handle_client_protocol_1_simple_std_net_tcp(
            &self,
            stream: &std::net::TcpStream,
            dictionary_file: &mut DictionaryFile,
            buffer: &mut [u8],
        );
    }

    impl ServerDebug for Server {
        fn send_bytes_debug_bad_condition(stream: &mut TcpStream, buffer: &[u8]) {
            let now = std::time::SystemTime::now();
            let unixtime = now.duration_since(std::time::UNIX_EPOCH).unwrap();
            let sec = unixtime.as_secs();
            let mod_base = 4;
            if sec % mod_base == 0 {
                ServerDebugImpl::send_half_each(stream, buffer);
            } else if sec % mod_base == 1 {
                ServerDebugImpl::send_split_random(stream, buffer);
            } else {
                stream.write_all_flush_ignore_error(buffer);
            }
        }

        fn send_bytes_debug(stream: &mut TcpStream, buffer: &[u8]) {
            stream.write_all_flush_ignore_error(buffer);
        }

        fn send_bytes_std_net_tcp(mut stream: &std::net::TcpStream, buffer: &[u8]) {
            if let Err(e) = stream.write_all(buffer) {
                Yaskkserv2::log_error(&format!("{e}"));
            }
            if let Err(e) = stream.flush() {
                Yaskkserv2::log_error(&format!("{e}"));
            }
        }

        fn handle_client_protocol_1_simple_std_net_tcp(
            &self,
            stream: &std::net::TcpStream,
            dictionary_file: &mut DictionaryFile,
            buffer: &mut [u8],
        ) {
            assert!(Self::validate_buffer_for_protocol_1_and_4(buffer), "error");
            match self.dictionary.read_candidates(dictionary_file, buffer) {
                Ok(mut candidates) => {
                    if Yaskkserv2::is_empty_candidates(&candidates) {
                        buffer[0] = b'4';
                        Self::send_bytes_std_net_tcp(stream, buffer);
                    } else {
                        candidates.push(b'\n');
                        Self::send_bytes_std_net_tcp(stream, &candidates);
                    }
                }
                Err(e) => panic!("error={e:?}"),
            }
        }
    }
}
