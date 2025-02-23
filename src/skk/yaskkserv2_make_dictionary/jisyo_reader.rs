use crate::skk::yaskkserv2_make_dictionary::{
    encoding_simple, Candidates, Dictionary, Encoding, EncodingOptions, JisyoEntriesMap,
    JisyoReader, Read, Seek, SkkError, Yaskkserv2MakeDictionary,
    JISYO_ENCODING_DETECT_BUFFER_LENGTH, JISYO_MAXIMUM_LINE_LENGTH,
    JISYO_MINIMUM_CANDIDATES_LENGTH, JISYO_MINIMUM_LINE_LENGTH,
};

use std::fs::File;
use std::io::{BufRead, BufReader};

trait BufReaderSkk {
    fn read_until_skk_jisyo(&mut self, buffer: &mut Vec<u8>) -> Result<usize, std::io::Error>;
}

impl BufReaderSkk for BufReader<std::fs::File> {
    fn read_until_skk_jisyo(&mut self, buffer: &mut Vec<u8>) -> Result<usize, std::io::Error> {
        fn find_cr_or_lf(buffer: &[u8]) -> Option<usize> {
            for (i, c) in buffer.iter().enumerate() {
                if *c == b'\n' || *c == b'\r' {
                    return Some(i);
                }
            }
            None
        }
        let mut read = 0;
        loop {
            let (done, used) = {
                let available = match self.fill_buf() {
                    Ok(n) => n,
                    Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                };
                #[allow(clippy::option_if_let_else)]
                if let Some(i) = find_cr_or_lf(available) {
                    buffer.extend_from_slice(&available[..=i]);
                    (true, i + 1)
                } else {
                    buffer.extend_from_slice(available);
                    (false, available.len())
                }
            };
            self.consume(used);
            read += used;
            if done || used == 0 {
                return Ok(read);
            }
        }
    }
}

impl JisyoReader {
    pub(in crate::skk) fn detect_jisyo_encoding(
        reader: &mut File,
    ) -> Result<(Encoding, EncodingOptions), SkkError> {
        let mut buffer = vec![0; JISYO_ENCODING_DETECT_BUFFER_LENGTH];
        let backup_seek_position = reader.seek(std::io::SeekFrom::Start(0))?;
        let read_length = reader.read(&mut buffer)?;
        reader.seek(std::io::SeekFrom::Start(backup_seek_position))?;
        if read_length < JISYO_MINIMUM_LINE_LENGTH {
            Yaskkserv2MakeDictionary::print_error("dictionary too short");
            return Err(SkkError::JisyoRead);
        }
        encoding_simple::Utility::detect_encoding(&buffer[..read_length])
    }

    pub(in crate::skk) fn get_merged_jisyo_entries_map(
        jisyo_full_paths: &[String],
        output_encoding: Encoding,
    ) -> Result<JisyoEntriesMap, SkkError> {
        Self::get_merged_jisyo_entries_map_core(jisyo_full_paths, Some(output_encoding))
    }

    #[cfg(test)]
    pub(in crate::skk) fn get_merged_jisyo_entries_map_no_encoding_conversion(
        jisyo_full_paths: &[String],
    ) -> Result<JisyoEntriesMap, SkkError> {
        Self::get_merged_jisyo_entries_map_core(jisyo_full_paths, None)
    }

    fn get_merged_jisyo_entries_map_core(
        jisyo_full_paths: &[String],
        output_encoding: Option<Encoding>,
    ) -> Result<JisyoEntriesMap, SkkError> {
        let mut result_jisyo_entries_map = JisyoEntriesMap::default();
        for jisyo_full_path in jisyo_full_paths {
            let mut reader = File::open(jisyo_full_path)?;
            let (jisyo_encoding, jisyo_encoding_options) =
                Self::detect_jisyo_encoding(&mut reader)?;
            if jisyo_encoding_options == EncodingOptions::Bom {
                Self::seek_skip_utf8_bom(&mut reader)?;
            }
            let mut buf_reader = BufReader::new(reader);
            let mut line_number = 1;
            let mut line = Vec::new();
            let mut is_last_cr = false;
            'INNER: while match buf_reader.read_until_skk_jisyo(&mut line) {
                Ok(0) | Err(_) => false,
                Ok(size) => {
                    let chomped_line = if is_last_cr && line[0] == b'\n' {
                        if size == 1 {
                            line.clear();
                            continue 'INNER;
                        }
                        &line[1..size - 1]
                    } else {
                        &line[..size - 1]
                    };
                    is_last_cr = line[size - 1] == b'\r';
                    if Self::is_skip_and_print_warning(
                        chomped_line,
                        jisyo_encoding,
                        jisyo_full_path,
                        &mut line_number,
                    ) {
                        line.clear();
                        continue 'INNER;
                    }
                    Self::update_jisyo_entries_map(
                        &mut result_jisyo_entries_map,
                        chomped_line,
                        jisyo_encoding,
                        output_encoding,
                        jisyo_full_path,
                        line_number,
                    )?;
                    line_number += 1;
                    line.clear();
                    true
                }
            } {}
        }
        Ok(result_jisyo_entries_map)
    }

    fn update_jisyo_entries_map(
        result_jisyo_entries_map: &mut JisyoEntriesMap,
        chomped_line: &[u8],
        jisyo_encoding: Encoding,
        output_encoding: Option<Encoding>,
        jisyo_full_path: &str,
        line_number: usize,
    ) -> Result<(), SkkError> {
        if let Ok(midashi_candidates) = Dictionary::get_midashi_candidates(chomped_line) {
            let (midashi, candidates) = Self::encode_midashi_candidates(
                midashi_candidates,
                jisyo_encoding,
                output_encoding,
            )?;
            let removed_duplicates_candidates = Candidates::remove_duplicates_bytes(&candidates);
            if candidates != removed_duplicates_candidates {
                Yaskkserv2MakeDictionary::print_warning(&format!(
                    r"CORRECTED! (DUPLICATE CANDIDATES) {jisyo_full_path}:{line_number} {:?}    {candidates:?} -> {removed_duplicates_candidates:?}",
                    &Self::get_line_string(chomped_line, jisyo_encoding)?,
                ));
            }
            #[allow(clippy::map_entry)]
            if result_jisyo_entries_map.contains_key(&midashi) {
                let merged_candidates = Candidates::merge_trimmed_slash_candidates(
                    Candidates::trim_one_slash(&result_jisyo_entries_map[&midashi]),
                    Candidates::trim_one_slash(&removed_duplicates_candidates),
                );
                result_jisyo_entries_map.insert(midashi, merged_candidates);
            } else {
                result_jisyo_entries_map.insert(midashi, removed_duplicates_candidates);
            }
        } else {
            Yaskkserv2MakeDictionary::print_warning(&format!(
                r"SKIPPED! (UNKNOWN FORMAT) {jisyo_full_path}:{line_number} {:?}",
                &Self::get_line_string(chomped_line, jisyo_encoding)?,
            ));
        }
        Ok(())
    }

    fn seek_skip_utf8_bom(reader: &mut File) -> Result<(), SkkError> {
        let mut bom_read_buffer = vec![0; 3];
        reader.read_exact(&mut bom_read_buffer)?;
        Ok(())
    }

    fn is_space_cr_lf_only(line: &[u8]) -> bool {
        !line
            .iter()
            .any(|v| *v != b' ' && *v != b'\r' && *v != b'\n')
    }

    fn is_skip_and_print_warning(
        chomped_line: &[u8],
        jisyo_encoding: Encoding,
        full_path: &str,
        line_number: &mut usize,
    ) -> bool {
        let mut print_skip_warning_and_add_line_number = |message: &str| {
            let line_string = Self::get_line_string(chomped_line, jisyo_encoding)
                .unwrap_or_else(|_| String::new());
            Yaskkserv2MakeDictionary::print_warning(&format!(
                "SKIPPED! ({}) {}:{} {:?}",
                message, full_path, line_number, &line_string
            ));
            *line_number += 1;
        };
        if chomped_line.is_empty() || Self::is_space_cr_lf_only(chomped_line) {
            print_skip_warning_and_add_line_number("EMPTY LINE");
            return true;
        }
        if chomped_line[0] == b';' {
            *line_number += 1;
            return true;
        }
        if chomped_line[0] == b' ' {
            print_skip_warning_and_add_line_number("BEGIN SPACE");
            return true;
        }
        if chomped_line[0] == b'\t' {
            print_skip_warning_and_add_line_number("BEGIN TAB");
            return true;
        }
        if chomped_line.len() < JISYO_MINIMUM_LINE_LENGTH {
            print_skip_warning_and_add_line_number("LINE TOO SHORT");
            return true;
        }
        if chomped_line.len() > JISYO_MAXIMUM_LINE_LENGTH {
            print_skip_warning_and_add_line_number("LINE TOO LONG");
            return true;
        }
        let Some(space) = twoway::find_bytes(chomped_line, b" ") else {
            print_skip_warning_and_add_line_number("SPACE NOT FOUND");
            return true;
        };
        if chomped_line.len() < space + JISYO_MINIMUM_CANDIDATES_LENGTH {
            print_skip_warning_and_add_line_number("CANDIDATES TOO SHORT");
            return true;
        }
        if chomped_line[space + 1] == b' ' {
            print_skip_warning_and_add_line_number("MULTI SPACE");
            return true;
        }
        if twoway::find_bytes(&chomped_line[space + 1..], b"//").is_some() {
            print_skip_warning_and_add_line_number("ILLEGAL CANDIDATES");
            return true;
        }
        false
    }

    fn get_line_string(line: &[u8], jisyo_encoding: Encoding) -> Result<String, SkkError> {
        if jisyo_encoding == Encoding::Utf8 {
            Ok(String::from_utf8(line.to_vec())?)
        } else {
            Ok(String::from_utf8(encoding_simple::Euc::decode(line)?)?)
        }
    }

    fn encode_midashi_candidates(
        midashi_candidates: (&[u8], &[u8]),
        jisyo_encoding: Encoding,
        output_encoding: Option<Encoding>,
    ) -> Result<(Vec<u8>, Vec<u8>), SkkError> {
        let midashi = if jisyo_encoding == Encoding::Euc || output_encoding.is_none() {
            midashi_candidates.0.to_vec()
        } else {
            encoding_simple::Euc::encode(midashi_candidates.0)?
        };
        let candidates =
            if output_encoding == Some(Encoding::Euc) && jisyo_encoding == Encoding::Utf8 {
                encoding_simple::Euc::encode(midashi_candidates.1)?
            } else if output_encoding == Some(Encoding::Utf8) && jisyo_encoding == Encoding::Euc {
                encoding_simple::Euc::decode(midashi_candidates.1)?
            } else {
                midashi_candidates.1.to_vec()
            };
        Ok((midashi, candidates))
    }
}
