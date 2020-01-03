use crate::skk::yaskkserv2::DictionaryReader;
use crate::skk::yaskkserv2_make_dictionary::*;

impl JisyoCreator {
    pub(in crate::skk) fn create(
        config: Config,
        output_jisyo_full_path: &str,
    ) -> Result<(), SkkError> {
        let mut reader = File::open(&config.dictionary_full_path)?;
        let on_memory = Dictionary::setup(SHA1_READ_BUFFER_LENGTH, &config.dictionary_full_path)?;
        let mut okuri_ari_map: BTreeMap<Vec<u8>, Vec<u8>> = BTreeMap::new();
        let mut okuri_nasi_map: BTreeMap<Vec<u8>, Vec<u8>> = BTreeMap::new();
        for dictionary_block_informations in on_memory.index_map.values() {
            for dictionary_block_information in dictionary_block_informations {
                Self::read_dictionary_block_information(
                    config.encoding,
                    &on_memory,
                    dictionary_block_information,
                    &mut reader,
                    &mut okuri_ari_map,
                    &mut okuri_nasi_map,
                )?;
            }
        }
        for dictionary_block_informations in &on_memory.index_ascii_hiragana_vec {
            for dictionary_block_information in dictionary_block_informations {
                Self::read_dictionary_block_information(
                    config.encoding,
                    &on_memory,
                    dictionary_block_information,
                    &mut reader,
                    &mut okuri_ari_map,
                    &mut okuri_nasi_map,
                )?;
            }
        }
        Self::write(
            output_jisyo_full_path,
            config.encoding,
            &okuri_ari_map,
            &okuri_nasi_map,
        )?;
        Ok(())
    }

    pub(in crate::skk) fn create_from_cache(
        input_cache_full_path: &str,
        output_jisyo_full_path: &str,
        output_jisyo_encoding: Encoding,
    ) -> Result<(), SkkError> {
        let okuri_ari_map: BTreeMap<Vec<u8>, Vec<u8>> = BTreeMap::new();
        let mut okuri_nasi_map: BTreeMap<Vec<u8>, Vec<u8>> = BTreeMap::new();
        for key_value in crate::skk::yaskkserv2::GoogleCache::read(input_cache_full_path)? {
            const EXPIRE_SECONDS_SKIP_LENGTH: usize = 1;
            let utf8_candidates = key_value
                .1
                .iter()
                .skip(EXPIRE_SECONDS_SKIP_LENGTH)
                .flat_map(|v| Candidates::quote_and_add_prefix(&v, Some(b'/')))
                .collect::<Vec<u8>>();
            let mut candidates = if output_jisyo_encoding == Encoding::Euc {
                encoding_simple::Euc::encode(&utf8_candidates).unwrap()
            } else {
                utf8_candidates
            };
            candidates.push(b'/');
            okuri_nasi_map.insert(key_value.0, candidates);
        }
        Self::write(
            output_jisyo_full_path,
            output_jisyo_encoding,
            &okuri_ari_map,
            &okuri_nasi_map,
        )?;
        Ok(())
    }

    fn read_dictionary_block_information(
        output_jisyo_encoding: Encoding,
        on_memory: &OnMemory,
        dictionary_block_information: &DictionaryBlockInformation,
        reader: &mut File,
        okuri_ari_map: &mut BTreeMap<Vec<u8>, Vec<u8>>,
        okuri_nasi_map: &mut BTreeMap<Vec<u8>, Vec<u8>>,
    ) -> Result<(), SkkError> {
        reader.seek(std::io::SeekFrom::Start(
            u64::from(on_memory.dictionary_fixed_header.blocks_offset)
                + u64::from(dictionary_block_information.offset),
        ))?;
        let mut buffer: Vec<u8> = vec![0; dictionary_block_information.length as usize];
        reader.read_exact(&mut buffer)?;
        Self::get_okuri_ari_nasi_maps(
            &buffer,
            output_jisyo_encoding,
            Encoding::from_u32(on_memory.dictionary_fixed_header.encoding),
            okuri_ari_map,
            okuri_nasi_map,
        )?;
        Ok(())
    }

    fn write(
        output_jisyo_full_path: &str,
        output_jisyo_encoding: Encoding,
        okuri_ari_map: &BTreeMap<Vec<u8>, Vec<u8>>,
        okuri_nasi_map: &BTreeMap<Vec<u8>, Vec<u8>>,
    ) -> Result<(), SkkError> {
        let mut writer = BufWriter::new(
            OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(output_jisyo_full_path)?,
        );
        if output_jisyo_encoding == Encoding::Euc {
            writer.write_all(b";; -*- mode: fundamental; coding: euc-jis-2004 -*-\n")?;
        } else {
            writer.write_all(b";; -*- mode: fundamental; coding: utf-8 -*-\n")?;
        }
        writer.write_all(b";; yaskkserv2 dictionary\n")?;
        writer.write_all(b";; okuri-ari entries.\n")?;
        for midashi in okuri_ari_map.keys().rev() {
            writer.write_all(midashi)?;
            writer.write_all(b" ")?;
            writer.write_all(&okuri_ari_map[midashi])?;
            writer.write_all(b"\n")?;
        }
        writer.write_all(b";; okuri-nasi entries.\n")?;
        for midashi_candidates in okuri_nasi_map {
            writer.write_all(midashi_candidates.0)?;
            writer.write_all(b" ")?;
            writer.write_all(midashi_candidates.1)?;
            writer.write_all(b"\n")?;
        }
        Ok(())
    }

    fn get_okuri_ari_nasi_maps(
        buffer: &[u8],
        output_jisyo_encoding: Encoding,
        dictionary_encoding: Encoding,
        okuri_ari_map: &mut BTreeMap<Vec<u8>, Vec<u8>>,
        okuri_nasi_map: &mut BTreeMap<Vec<u8>, Vec<u8>>,
    ) -> Result<(), SkkError> {
        let mut offset = 0;
        while let Some(space_find) = twoway::find_bytes(&buffer[offset..], b" ") {
            if let Some(lf_find) = twoway::find_bytes(&buffer[offset + space_find..], b"\n") {
                const TOP_LF_LENGTH: usize = 1;
                const SPACE_LENGTH: usize = 1;
                let candidates_slice =
                    &buffer[offset + space_find + SPACE_LENGTH..offset + space_find + lf_find];
                let candidates = if output_jisyo_encoding == Encoding::Euc
                    && dictionary_encoding == Encoding::Utf8
                {
                    encoding_simple::Euc::encode(candidates_slice)?
                } else if output_jisyo_encoding == Encoding::Utf8
                    && dictionary_encoding == Encoding::Euc
                {
                    encoding_simple::Euc::decode(candidates_slice)?
                } else {
                    candidates_slice.to_vec()
                };
                let euc_midashi_slice = &buffer[offset + TOP_LF_LENGTH..offset + space_find];
                let midashi = if output_jisyo_encoding == Encoding::Euc {
                    euc_midashi_slice.to_vec()
                } else {
                    encoding_simple::Euc::decode(euc_midashi_slice)?
                };
                if DictionaryReader::is_okuri_ari(&euc_midashi_slice) {
                    okuri_ari_map.insert(midashi, candidates);
                } else {
                    okuri_nasi_map.insert(midashi, candidates);
                }
                offset += space_find + lf_find;
            } else {
                return Err(SkkError::BrokenDictionary);
            }
        }
        Ok(())
    }
}
