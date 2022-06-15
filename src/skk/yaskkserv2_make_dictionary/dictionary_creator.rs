use crate::skk::yaskkserv2_make_dictionary::{
    BlockInformationOffsetLength, Config, Dictionary, DictionaryBlockHeads,
    DictionaryBlockInformation, DictionaryCreator, DictionaryFixedHeader, Digest, Encoding,
    IndexDataHeader, IndexDataHeaderBlockHeader, JisyoReader, OpenOptions, Sha1, SkkError,
    TemporaryBlockMap, ToFromNeBytes, Write, Yaskkserv2MakeDictionary, BLOCK_ALIGNMENT_LENGTH,
    DICTIONARY_BLOCK_UNIT_LENGTH, DICTIONARY_FIXED_HEADER_AREA_LENGTH, DICTIONARY_VERSION,
    HEADER_ALIGNMENT_LENGTH, INDEX_DATA_BLOCK_LENGTH, SHA1SUM_ZERO,
};

#[cfg(feature = "assert_paranoia")]
use crate::{const_assert, const_assert_eq, const_panic};

struct CreateIndexDataResult {
    index_data_header: Vec<u8>,
    index_data: Vec<u8>,
    blocks: Vec<u8>,
}

impl DictionaryCreator {
    pub(in crate::skk) fn create(
        config: &Config,
        encoding_table: &[u8],
        jisyo_full_paths: &[String],
    ) -> Result<(), SkkError> {
        let create_index_data_result = Self::create_index_data(jisyo_full_paths, config.encoding)?;
        let mut hasher = Sha1::new();
        let (mut dictionary_fixed_header, blocks_padding_length) =
            Self::create_dictionary_fixed_header(
                encoding_table.len(),
                create_index_data_result.index_data_header.len(),
                create_index_data_result.index_data.len(),
                create_index_data_result.blocks.len(),
                config.encoding,
            );
        let bytes_dictionary_fixed_header = dictionary_fixed_header.to_ne_bytes();
        let mut dictionary_fixed_header_area: [u8; DICTIONARY_FIXED_HEADER_AREA_LENGTH as usize] =
            [0; DICTIONARY_FIXED_HEADER_AREA_LENGTH as usize];
        dictionary_fixed_header_area[..bytes_dictionary_fixed_header.len()]
            .copy_from_slice(&bytes_dictionary_fixed_header);
        Self::write_and_calculate_hash(
            &config.dictionary_full_path,
            &mut hasher,
            &dictionary_fixed_header_area,
            encoding_table,
            &create_index_data_result,
            blocks_padding_length,
        )?;
        Self::write_hash(
            &config.dictionary_full_path,
            &mut hasher,
            &mut dictionary_fixed_header,
        )?;
        if config.is_verbose {
            Self::print_verbose(&create_index_data_result, encoding_table.len());
        }
        Ok(())
    }

    #[allow(clippy::cast_possible_truncation)]
    const fn create_dictionary_fixed_header(
        encoding_table_length: usize,
        index_data_header_length: usize,
        index_data_length: usize,
        blocks_length: usize,
        encoding: Encoding,
    ) -> (DictionaryFixedHeader, u32) {
        let raw_blocks_offset = (DICTIONARY_FIXED_HEADER_AREA_LENGTH as usize
            + encoding_table_length
            + index_data_header_length
            + index_data_length) as u32;
        let aligned_blocks_offset = Self::get_header_aligned_length(raw_blocks_offset);
        let blocks_padding_length = aligned_blocks_offset - raw_blocks_offset;
        let encoding_table_offset = DICTIONARY_FIXED_HEADER_AREA_LENGTH as usize;
        let index_data_header_offset = encoding_table_offset + encoding_table_length;
        let index_data_offset = index_data_header_offset + index_data_header_length;
        let dictionary_length = aligned_blocks_offset as usize + blocks_length;
        #[cfg(feature = "assert_paranoia")]
        {
            const_assert_eq!(
                index_data_offset,
                encoding_table_offset + encoding_table_length + index_data_header_length
            );
        }
        (
            DictionaryFixedHeader {
                dictionary_version: DICTIONARY_VERSION,
                encoding_table_offset: encoding_table_offset as u32,
                encoding_table_length: encoding_table_length as u32,
                index_data_header_offset: index_data_header_offset as u32,
                index_data_header_length: index_data_header_length as u32,
                index_data_offset: index_data_offset as u32,
                index_data_length: index_data_length as u32,
                blocks_offset: aligned_blocks_offset,
                blocks_length: blocks_length as u32,
                dictionary_length: dictionary_length as u32,
                encoding: encoding as u32,
                sha1sum: SHA1SUM_ZERO,
            },
            blocks_padding_length,
        )
    }

    fn print_verbose(
        create_index_data_result: &CreateIndexDataResult,
        encoding_table_length: usize,
    ) {
        println!(
            "       encoding_table length = {} bytes",
            encoding_table_length
        );
        println!(
            "    index_data_header length = {} bytes",
            create_index_data_result.index_data_header.len()
        );
        println!(
            "           index_data length = {} bytes",
            create_index_data_result.index_data.len()
        );
        println!(
            "               blocks length = {} bytes",
            create_index_data_result.blocks.len()
        );
    }

    // buffer 内の entry ("\nMIDASHI /CANDIDATES/\n") の最大長を取得する
    fn get_max_entry_length(buffer: &[u8]) -> Result<usize, SkkError> {
        const SKIP_LF_LENGTH: usize = 1;
        let mut offset = 0;
        let mut max_entry_length = 0;
        loop {
            // offset は先頭の "\n" を指しているので offset + SKIP_LF_LENGTH から終端の "\n" を探す
            if let Some(find) = twoway::find_bytes(&buffer[offset + SKIP_LF_LENGTH..], b"\n") {
                let length = SKIP_LF_LENGTH + find + SKIP_LF_LENGTH;
                if max_entry_length < length {
                    max_entry_length = length;
                }
                if SKIP_LF_LENGTH + offset + find + SKIP_LF_LENGTH == buffer.len() {
                    break;
                }
                offset = offset + find + SKIP_LF_LENGTH;
            } else {
                Yaskkserv2MakeDictionary::print_error("dictionary format X");
                return Err(SkkError::JisyoRead);
            }
        }
        Ok(max_entry_length)
    }

    #[allow(clippy::cast_possible_truncation)]
    fn get_dictionary_block_informations(
        block_buffer: &[u8],
        blocks_len: usize,
        aligned_block: &mut Vec<u8>,
    ) -> Result<Vec<DictionaryBlockInformation>, SkkError> {
        let mut dictionary_block_informations = Vec::new();
        // src/skk/yaskkserv2_make_dictionary/mod.rs の string blocks の解説にもあるように、
        // buffer に含まれる各 entry の前後には必ず b'\n' が含まれている。
        let block_buffer_length = block_buffer.len();
        let mut offset = 0;
        let max_entry_length = Self::get_max_entry_length(block_buffer)?;
        loop {
            let mut unit_length = DICTIONARY_BLOCK_UNIT_LENGTH;
            // offset は先頭の "\n" を指しているので、 midashi 部分を抽出するため space を探す
            if let Some(find) = twoway::find_bytes(&block_buffer[offset..], b" ") {
                let mut should_break = offset + unit_length >= block_buffer_length;
                if should_break {
                    unit_length = block_buffer_length - offset;
                }
                if let Some(rfind) =
                    twoway::rfind_bytes(&block_buffer[..(offset + unit_length)], b"\n")
                {
                    // rfind が offset ということは先頭の "\n" まで探索してしまった、すなわち
                    // 探すべき要素が見付からなかったということになるので unit_length を最大値で
                    // 補正。
                    if rfind == offset {
                        unit_length = max_entry_length;
                        if block_buffer_length == unit_length {
                            should_break = true;
                        }
                    }
                }
                // unit_length だけざっくりと進めた位置から (euc/utf8 境界でない位置も指す)
                // "\n" まで戻って offset を先頭に補正 (euc/utf8 的にも絶対に正しい位置)
                if let Some(rfind) =
                    twoway::rfind_bytes(&block_buffer[..(offset + unit_length)], b"\n")
                {
                    const START_LF_LENGTH: usize = 1;
                    const END_LF_LENGTH: usize = 1;
                    aligned_block.extend_from_slice(&block_buffer[offset..rfind]);
                    dictionary_block_informations.push(DictionaryBlockInformation {
                        midashi: block_buffer[(offset + START_LF_LENGTH)..(offset + find)].to_vec(),
                        offset: (blocks_len + offset) as u32,
                        length: (rfind + END_LF_LENGTH - offset) as u32,
                    });
                    if should_break {
                        aligned_block.push(b'\n');
                        let aligned_length =
                            Self::get_block_aligned_length(aligned_block.len() as u32);
                        let padding = vec![b'X'; aligned_length as usize - aligned_block.len()];
                        aligned_block.extend_from_slice(&padding);
                        break;
                    }
                    offset = rfind;
                } else {
                    Yaskkserv2MakeDictionary::print_error("dictionary format A");
                    return Err(SkkError::JisyoRead);
                }
            } else {
                Yaskkserv2MakeDictionary::print_error("dictionary format B");
                return Err(SkkError::JisyoRead);
            }
        }
        // vector は頭から探索できるよう逆順に並ぶことに注意
        dictionary_block_informations.reverse();
        #[cfg(feature = "print_debug_for_benchmark")]
        {
            Self::print_debug_dictionary_block_informations(&dictionary_block_informations);
        }
        Ok(dictionary_block_informations)
    }

    #[cfg(feature = "print_debug_for_benchmark")]
    fn print_debug_dictionary_block_informations(
        dictionary_block_informations: &[DictionaryBlockInformation],
    ) {
        let last_midashi = &dictionary_block_informations.last().unwrap().midashi;
        if dictionary_block_informations
            .iter()
            .find(|v| v.length > DICTIONARY_BLOCK_UNIT_LENGTH as u32)
            .is_some()
            || dictionary_block_informations.len() > 200
            || last_midashi == b"c"
            || last_midashi == b"s"
        {
            println!("len() : {}", dictionary_block_informations.len());
            println!(
                "next() : {:?} length={} {:x?}",
                String::from_utf8(
                    encoding_simple::Euc::decode(
                        &dictionary_block_informations.iter().next().unwrap().midashi
                    )
                    .unwrap()
                ),
                dictionary_block_informations.iter().next().unwrap().length,
                &dictionary_block_informations.iter().next().unwrap().midashi
            );
            println!(
                "last() : {:?} length={} {:x?}",
                String::from_utf8(
                    encoding_simple::Euc::decode(
                        &dictionary_block_informations.iter().last().unwrap().midashi
                    )
                    .unwrap()
                ),
                dictionary_block_informations.iter().last().unwrap().length,
                dictionary_block_informations.iter().last().unwrap().midashi
            );
            for u in dictionary_block_informations {
                println!(
                    "    {:?} LENGTH={} {:x?}",
                    String::from_utf8(encoding_simple::Euc::decode(&u.midashi).unwrap()),
                    u.length,
                    &u.midashi
                );
            }
        }
    }

    fn create_temporary_block_map(
        jisyo_full_paths: &[String],
        output_encoding: Encoding,
    ) -> Result<TemporaryBlockMap, SkkError> {
        let mut temporary_block_map = TemporaryBlockMap::new();
        for jisyo_line in
            JisyoReader::get_merged_jisyo_entries_map(jisyo_full_paths, output_encoding)?
        {
            let dictionary_midashi_key = Dictionary::get_dictionary_midashi_key(&jisyo_line.0)?;
            let mut entry = Vec::new();
            entry.extend_from_slice(&jisyo_line.0);
            entry.extend_from_slice(b" ");
            entry.extend_from_slice(&jisyo_line.1);
            entry.extend_from_slice(b"\n");
            (*temporary_block_map
                .entry(dictionary_midashi_key)
                .or_insert_with(|| vec![b'\n']))
            .extend_from_slice(&entry);
        }
        Ok(temporary_block_map)
    }

    #[allow(clippy::cast_possible_truncation)]
    fn create_index_data(
        jisyo_full_paths: &[String],
        output_encoding: Encoding,
    ) -> Result<CreateIndexDataResult, SkkError> {
        let mut index_data_header: Vec<u8> = Vec::new();
        // block_header_length は確定後に書き込むので一旦 default() を書き込んでおくことに注意
        index_data_header.extend_from_slice(&IndexDataHeader::default().to_ne_bytes());
        let mut block_header_length: u32 = 0;
        let mut block_offset = 0;
        let mut block_length = 0;
        let mut block_unit_length: u32 = 0;
        let mut previous_length = 0;
        let mut index_data = Vec::new();
        let mut blocks = Vec::new();
        for dictionary_midashi_key_and_block_buffer in
            Self::create_temporary_block_map(jisyo_full_paths, output_encoding)?
        {
            let mut aligned_block = Vec::new();
            let dictionary_block_informations = Self::get_dictionary_block_informations(
                &dictionary_midashi_key_and_block_buffer.1,
                blocks.len(),
                &mut aligned_block,
            )?;
            let joined_midashi = dictionary_block_informations
                .iter()
                .map(|v| v.midashi.clone())
                .collect::<Vec<Vec<u8>>>()
                .join(&b' ');
            index_data.extend_from_slice(
                &DictionaryBlockHeads {
                    information_length: dictionary_block_informations.len() as u32,
                    information_midashi_length: joined_midashi.len() as u32,
                    dictionary_midashi_key: dictionary_midashi_key_and_block_buffer.0,
                }
                .to_ne_bytes(),
            );
            index_data.extend_from_slice(
                &dictionary_block_informations
                    .iter()
                    .flat_map(|v| {
                        BlockInformationOffsetLength {
                            offset: v.offset,
                            length: v.length,
                        }
                        .to_ne_bytes()
                    })
                    .collect::<Vec<u8>>(),
            );
            index_data.extend_from_slice(&joined_midashi);
            blocks.extend_from_slice(&aligned_block);
            let current_index_data_length = index_data.len() - previous_length;
            previous_length = index_data.len();
            if block_length + current_index_data_length >= INDEX_DATA_BLOCK_LENGTH {
                block_header_length += 1;
                index_data_header.extend_from_slice(
                    &IndexDataHeaderBlockHeader {
                        offset: block_offset as u32,
                        length: block_length as u32,
                        unit_length: block_unit_length,
                    }
                    .to_ne_bytes(),
                );
                block_offset = index_data.len() - current_index_data_length;
                block_length = current_index_data_length;
                block_unit_length = 1;
            } else {
                block_length += current_index_data_length;
                block_unit_length += 1;
            }
        }
        if block_length > 0 {
            block_header_length += 1;
            index_data_header.extend_from_slice(
                &IndexDataHeaderBlockHeader {
                    offset: block_offset as u32,
                    length: block_length as u32,
                    unit_length: block_unit_length,
                }
                .to_ne_bytes(),
            );
        }
        // block_unit_length が確定したので default() で書き込んでいた値を上書きすることに注意
        index_data_header[..std::mem::size_of::<IndexDataHeader>()].copy_from_slice(
            &IndexDataHeader {
                block_buffer_length: INDEX_DATA_BLOCK_LENGTH as u32,
                block_header_length,
            }
            .to_ne_bytes(),
        );
        Ok(CreateIndexDataResult {
            index_data_header,
            index_data,
            blocks,
        })
    }

    const fn get_block_aligned_length(length: u32) -> u32 {
        length + (BLOCK_ALIGNMENT_LENGTH - (length % BLOCK_ALIGNMENT_LENGTH))
    }

    const fn get_header_aligned_length(length: u32) -> u32 {
        length + (HEADER_ALIGNMENT_LENGTH - (length % HEADER_ALIGNMENT_LENGTH))
    }

    fn write_and_calculate_hash(
        dictionary_full_path: &str,
        hasher: &mut Sha1,
        dictionary_fixed_header_area: &[u8; DICTIONARY_FIXED_HEADER_AREA_LENGTH as usize],
        encoding_table: &[u8],
        create_index_data_result: &CreateIndexDataResult,
        blocks_padding_length: u32,
    ) -> Result<(), SkkError> {
        let mut writer = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(dictionary_full_path)?;
        writer.write_all(dictionary_fixed_header_area)?;
        hasher.update(dictionary_fixed_header_area);
        writer.write_all(encoding_table)?;
        hasher.update(encoding_table);
        writer.write_all(&create_index_data_result.index_data_header)?;
        hasher.update(&create_index_data_result.index_data_header);
        writer.write_all(&create_index_data_result.index_data)?;
        hasher.update(&create_index_data_result.index_data);
        let padding = vec![0; blocks_padding_length as usize];
        writer.write_all(&padding)?;
        hasher.update(&padding);
        writer.write_all(&create_index_data_result.blocks)?;
        hasher.update(&create_index_data_result.blocks);
        writer.flush()?;
        Ok(())
    }

    fn write_hash(
        dictionary_full_path: &str,
        hasher: &mut Sha1,
        dictionary_fixed_header: &mut DictionaryFixedHeader,
    ) -> Result<(), SkkError> {
        let mut writer = OpenOptions::new().write(true).open(dictionary_full_path)?;
        let digest: [u8; 20] = hasher.clone().finalize().as_slice().try_into().unwrap();
        dictionary_fixed_header.sha1sum.copy_from_slice(&digest);
        writer.write_all(&dictionary_fixed_header.to_ne_bytes())?;
        writer.flush()?;
        Ok(())
    }
}

#[cfg(test)]
pub(in crate::skk) mod test_unix {
    use rand::Rng;

    use crate::skk::yaskkserv2_make_dictionary::dictionary_creator::{
        DictionaryCreator, TemporaryBlockMap,
    };

    fn get_random_ascii_vec(length: usize) -> Vec<u8> {
        let mut ascii_vec = Vec::new();
        let rand_length = rand::thread_rng().gen_range(1, length + 1);
        for _ in 0..rand_length {
            ascii_vec.push(rand::thread_rng().gen_range(b'0', b'9' + 1));
        }
        ascii_vec
    }

    #[test]
    fn yaskkserv2_dictionary_creator_get_dictionary_block_informations_test() {
        const LOOP: usize = 100;
        const ENTRIES: usize = 32 * 1024;
        for _ in 0..LOOP {
            let dictionary_midashi_key = [b'0', 0, 0, 0];
            let mut temporary_block_map = TemporaryBlockMap::new();
            {
                const MIDASHI_LENGTH: usize = 32;
                const CANDIDATES_LENGTH: usize = 1024;
                let mut line = vec![b'\n'];
                for _ in 0..ENTRIES {
                    line.extend_from_slice(&get_random_ascii_vec(MIDASHI_LENGTH));
                    line.push(b' ');
                    line.extend_from_slice(&get_random_ascii_vec(CANDIDATES_LENGTH));
                    line.push(b'\n');
                }
                temporary_block_map.insert(dictionary_midashi_key, line);
            }
            let blocks_len = 0;
            let mut aligned_block = Vec::new();
            let dictionary_block_informations =
                DictionaryCreator::get_dictionary_block_informations(
                    &temporary_block_map[&dictionary_midashi_key],
                    blocks_len,
                    &mut aligned_block,
                )
                .unwrap();
            println!(
                "dictionary_block_informations.len()={}",
                dictionary_block_informations.len()
            );
        }
    }
}
