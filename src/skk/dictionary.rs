use sha1::Sha1;
use std::fs::File;
use std::io::{Read, Seek};

use crate::skk::{
    once_init_encoding_table, BlockInformationOffsetLength, Dictionary, DictionaryBlockHeads,
    DictionaryBlockInformation, DictionaryFixedHeader, DictionaryMidashiKey, IndexAsciiHiraganaVec,
    IndexDataHeader, IndexDataHeaderBlockHeader, IndexMap, OnMemory, SkkError, ToFromNeBytes,
    DICTIONARY_FIXED_HEADER_AREA_LENGTH, INDEX_ASCII_HIRAGANA_VEC_LENGTH, SHA1SUM_LENGTH,
    SHA1SUM_ZERO,
};

impl Dictionary {
    pub(in crate::skk) fn setup(
        sha1_read_buffer_length: usize,
        dictionary_full_path: &str,
    ) -> Result<OnMemory, SkkError> {
        let mut reader = File::open(dictionary_full_path)?;
        let mut hasher = Sha1::new();
        let mut buffer = vec![0; DICTIONARY_FIXED_HEADER_AREA_LENGTH as usize];
        reader.read_exact(&mut buffer)?;
        let mut dictionary_fixed_header: DictionaryFixedHeader =
            DictionaryFixedHeader::from_ne_bytes(&buffer);
        let sha1sum = dictionary_fixed_header.sha1sum;
        dictionary_fixed_header.sha1sum = SHA1SUM_ZERO;
        {
            let bytes_dictionary_fixed_header = dictionary_fixed_header.to_ne_bytes();
            buffer[..bytes_dictionary_fixed_header.len()]
                .copy_from_slice(&bytes_dictionary_fixed_header);
        }
        let buffer = buffer;
        hasher.update(&buffer);
        Self::validate_except_dictionary_fixed_header(
            sha1_read_buffer_length,
            dictionary_full_path,
            &mut hasher,
            u64::from(dictionary_fixed_header.dictionary_length),
            &sha1sum,
        )?;
        reader.seek(std::io::SeekFrom::Start(u64::from(
            dictionary_fixed_header.encoding_table_offset,
        )))?;
        let mut buffer = vec![0; dictionary_fixed_header.encoding_table_length as usize];
        reader.read_exact(&mut buffer)?;
        let buffer = buffer;
        once_init_encoding_table(&buffer);
        reader.seek(std::io::SeekFrom::Start(u64::from(
            dictionary_fixed_header.index_data_header_offset,
        )))?;
        let mut buffer = vec![0; dictionary_fixed_header.index_data_header_length as usize];
        reader.read_exact(&mut buffer)?;
        let buffer = buffer;
        let index_data_offset = dictionary_fixed_header.index_data_offset;
        let (index_map, index_ascii_hiragana_vec) =
            Self::create_index_map_and_index_ascii_hiragana_vec(
                index_data_offset,
                &mut reader,
                &buffer,
            )?;
        Ok(OnMemory {
            dictionary_fixed_header,
            index_map,
            index_ascii_hiragana_vec,
        })
    }

    /// `euc_buffer` から `DictionaryMidashiKey` を取得する
    ///
    /// なお、ここでの `euc_buffer` は正しい euc code ということを前提として良い。
    /// (なので `euc_buffer[0]` の範囲が正当ならば 2 bytes 目の内容を見ずに判定して良いし、
    /// 2 bytes 目以降は存在するとしてアクセスしても良い)
    pub(in crate::skk) const fn get_dictionary_midashi_key(
        euc_buffer: &[u8],
    ) -> Result<DictionaryMidashiKey, SkkError> {
        match euc_buffer[0] {
            0xa1..=0xfe | 0x8e => Ok([euc_buffer[0], euc_buffer[1], 0, 0]),
            0x00..=0x7f => Ok([euc_buffer[0], 0, 0, 0]),
            0x8f => Ok([euc_buffer[0], euc_buffer[1], euc_buffer[2], 0]),
            _ => Err(SkkError::Encoding),
        }
    }

    pub(in crate::skk) fn get_midashi_candidates(
        buffer: &[u8],
    ) -> Result<(&[u8], &[u8]), SkkError> {
        let space = twoway::find_bytes(buffer, b" ").ok_or(SkkError::JisyoRead)?;
        let last_slash = twoway::rfind_bytes(buffer, b"/").ok_or(SkkError::JisyoRead)?;
        Ok((&buffer[..space], &buffer[space + 1..=last_slash]))
    }

    fn validate_except_dictionary_fixed_header(
        sha1_read_buffer_length: usize,
        dictionary_full_path: &str,
        hasher: &mut Sha1,
        dictionary_length: u64,
        sha1sum: &[u8; SHA1SUM_LENGTH],
    ) -> Result<(), SkkError> {
        let mut reader = File::open(dictionary_full_path)?;
        let mut buffer: Vec<u8> = vec![0; sha1_read_buffer_length];
        let mut total_scan_length = u64::from(DICTIONARY_FIXED_HEADER_AREA_LENGTH);
        reader.seek(std::io::SeekFrom::Start(total_scan_length))?;
        let mut handle = reader.take(dictionary_length - total_scan_length);
        loop {
            let read_length = handle.read(&mut buffer)? as u64;
            if read_length == 0 {
                if total_scan_length != dictionary_length {
                    return Err(SkkError::BrokenDictionary);
                }
                break;
            }
            #[allow(clippy::cast_possible_truncation)]
            hasher.update(&buffer[..(read_length as usize)]);
            total_scan_length += read_length;
        }
        if *sha1sum != hasher.digest().bytes() {
            return Err(SkkError::BrokenDictionary);
        }
        Ok(())
    }

    fn get_joined_midashi(
        buffer: &[u8],
        buffer_offset: usize,
        dictionary_block_informations_length: usize,
        joined_midashi_length: usize,
    ) -> Vec<&[u8]> {
        let joined_midashi_offset = buffer_offset
            + std::mem::size_of::<DictionaryBlockHeads>()
            + dictionary_block_informations_length
                * std::mem::size_of::<BlockInformationOffsetLength>();
        buffer[joined_midashi_offset..joined_midashi_offset + joined_midashi_length]
            .split(|&v| v == b' ')
            .collect::<Vec<&[u8]>>()
    }

    fn update_index_map_and_index_ascii_hiragana_vec(
        index_data_header_block_header: &IndexDataHeaderBlockHeader,
        buffer: &[u8],
        result_index_map: &mut IndexMap,
        result_index_ascii_hiragana_vec: &mut IndexAsciiHiraganaVec,
    ) {
        let mut buffer_offset = 0;
        for _ in 0..index_data_header_block_header.unit_length {
            let dictionary_block_heads =
                DictionaryBlockHeads::from_ne_bytes(&buffer[buffer_offset..]);
            let ascii_hiragana_vec_index = OnMemory::get_ascii_hiragana_vec_index(
                dictionary_block_heads.dictionary_midashi_key,
            );
            let joined_midashi = Self::get_joined_midashi(
                buffer,
                buffer_offset,
                dictionary_block_heads.information_length as usize,
                dictionary_block_heads.information_midashi_length as usize,
            );
            assert_eq!(
                joined_midashi.len(),
                dictionary_block_heads.information_length as usize
            );
            let mut dictionary_block_informations = Vec::new();
            for (i, midashi) in joined_midashi.iter().enumerate() {
                let block_information_offset_length = BlockInformationOffsetLength::from_ne_bytes(
                    &buffer[buffer_offset
                        + std::mem::size_of::<DictionaryBlockHeads>()
                        + i * std::mem::size_of::<BlockInformationOffsetLength>()..],
                );
                let dictionary_block_information = DictionaryBlockInformation {
                    midashi: midashi.to_vec(),
                    offset: block_information_offset_length.offset,
                    length: block_information_offset_length.length,
                };
                dictionary_block_informations.push(dictionary_block_information);
            }
            if let Some(index) = ascii_hiragana_vec_index {
                result_index_ascii_hiragana_vec[index] = dictionary_block_informations;
            } else {
                result_index_map.insert(
                    dictionary_block_heads.dictionary_midashi_key,
                    dictionary_block_informations,
                );
            }
            buffer_offset += std::mem::size_of::<DictionaryBlockHeads>()
                + dictionary_block_heads.information_length as usize
                    * std::mem::size_of::<BlockInformationOffsetLength>()
                + dictionary_block_heads.information_midashi_length as usize;
        }
    }

    fn create_index_map_and_index_ascii_hiragana_vec(
        index_data_offset: u32,
        reader: &mut File,
        header_buffer: &[u8],
    ) -> Result<(IndexMap, IndexAsciiHiraganaVec), SkkError> {
        const BLOCK_BUFFER_LENGTH_LIMIT: usize = 2 * 1024 * 1024;
        let index_data_header = IndexDataHeader::from_ne_bytes(header_buffer);
        assert!((index_data_header.block_buffer_length as usize) < BLOCK_BUFFER_LENGTH_LIMIT);
        let mut buffer = vec![0; index_data_header.block_buffer_length as usize];
        let mut result_index_map = IndexMap::default();
        let mut result_index_ascii_hiragana_vec: IndexAsciiHiraganaVec =
            vec![Vec::new(); INDEX_ASCII_HIRAGANA_VEC_LENGTH];
        for i in 0..index_data_header.block_header_length as usize {
            let index_data_header_block_header = IndexDataHeaderBlockHeader::from_ne_bytes(
                &header_buffer[std::mem::size_of::<IndexDataHeader>()
                    + i * std::mem::size_of::<IndexDataHeaderBlockHeader>()..],
            );
            reader.seek(std::io::SeekFrom::Start(u64::from(
                index_data_offset + index_data_header_block_header.offset,
            )))?;
            reader.read_exact(&mut buffer[..index_data_header_block_header.length as usize])?;
            Self::update_index_map_and_index_ascii_hiragana_vec(
                &index_data_header_block_header,
                &buffer,
                &mut result_index_map,
                &mut result_index_ascii_hiragana_vec,
            );
        }
        Ok((result_index_map, result_index_ascii_hiragana_vec))
    }
}
