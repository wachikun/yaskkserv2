use sha1::{Digest, Sha1};
use std::fs::File;
use std::io::{Read, Write};

use crate::skk::yaskkserv2::{
    GoogleCache, GoogleCacheBTreeMap, SkkError, GOOGLE_CACHE_OBJECT, SHA1SUM_LENGTH,
};

impl GoogleCache {
    pub(in crate::skk) fn get_candidates(midashi: &[u8]) -> Vec<Vec<u8>> {
        let mut cached_candidates: Vec<Vec<u8>> = Vec::new();
        if GOOGLE_CACHE_OBJECT
            .read()
            .unwrap()
            .map
            .contains_key(midashi)
            && GOOGLE_CACHE_OBJECT.read().unwrap().map[midashi].len() >= 2
        {
            cached_candidates.clone_from(&GOOGLE_CACHE_OBJECT.read().unwrap().map[midashi]);
            cached_candidates.remove(0);
        }
        cached_candidates
    }

    pub(in crate::skk) fn write_candidates(
        midashi: &[u8],
        candidates: &[Vec<u8>],
        cache_full_path: &str,
        cache_entries: usize,
        cache_expire_seconds: u64,
    ) -> Result<(), SkkError> {
        let should_write = if GOOGLE_CACHE_OBJECT
            .write()
            .unwrap()
            .map
            .contains_key(midashi)
        {
            candidates.to_vec() != GOOGLE_CACHE_OBJECT.write().unwrap().map[midashi]
        } else {
            true
        };
        if should_write {
            let mut cache_candidates = candidates.to_owned();
            let unix_time_now = std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            cache_candidates.insert(0, unix_time_now.to_string().as_bytes().to_vec());
            GOOGLE_CACHE_OBJECT
                .write()
                .unwrap()
                .map
                .insert(midashi.to_vec(), cache_candidates);
            let mut expired_map: GoogleCacheBTreeMap = GOOGLE_CACHE_OBJECT
                .write()
                .unwrap()
                .map
                .clone()
                .into_iter()
                .filter(|(_k, v)| {
                    let time: u64 = Self::parse_or_zero_u64(&v[0]);
                    time > unix_time_now - cache_expire_seconds
                })
                .collect();
            if expired_map.keys().len() > cache_entries {
                let min = expired_map.clone().into_iter().min_by(|a, b| {
                    let au: u64 = Self::parse_or_zero_u64(&a.1[0]);
                    let bu: u64 = Self::parse_or_zero_u64(&b.1[0]);
                    au.cmp(&bu)
                });
                if let Some(m) = min {
                    expired_map.remove(&m.0);
                }
            }
            GOOGLE_CACHE_OBJECT.write().unwrap().map = expired_map.clone();
            Self::write(cache_full_path, &expired_map)?;
        }
        Ok(())
    }

    pub(in crate::skk) fn setup_use_rwlock_internally(
        google_cache_full_path: &str,
    ) -> Result<(), SkkError> {
        {
            let Ok(mut rw_lock_write) = GOOGLE_CACHE_OBJECT.write() else {
                return Err(SkkError::CacheOpen);
            };
            rw_lock_write.map =
                Self::read(google_cache_full_path).unwrap_or_else(|_| GoogleCacheBTreeMap::new());
        }
        Ok(())
    }

    pub(in crate::skk) fn read(cache_full_path: &str) -> Result<GoogleCacheBTreeMap, SkkError> {
        let Ok(mut file) = File::open(cache_full_path) else {
            return Err(SkkError::CacheOpen);
        };
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        let buffer = buffer;
        let empty_map = GoogleCacheBTreeMap::new();
        let serialized_empty_map = Self::serialize(&empty_map)?;
        let rough_empty_buffer_length = SHA1SUM_LENGTH + serialized_empty_map.len();
        if buffer.len() >= rough_empty_buffer_length {
            let mut hasher = Sha1::new();
            hasher.update(&buffer[SHA1SUM_LENGTH..]);
            let digest: [u8; SHA1SUM_LENGTH] = hasher.finalize().as_slice().try_into().unwrap();
            if digest == buffer[..SHA1SUM_LENGTH] {
                return Self::deserialize(&buffer[SHA1SUM_LENGTH..]);
            }
        }
        Err(SkkError::BrokenCache)
    }

    fn serialize<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, SkkError> {
        bincode::serialize(value).map_err(SkkError::Bincode)
    }

    fn deserialize<'a, T: serde::de::Deserialize<'a>>(bytes: &'a [u8]) -> Result<T, SkkError> {
        bincode::deserialize(bytes).map_err(SkkError::Bincode)
    }

    fn parse_or_zero_u64(bytes: &[u8]) -> u64 {
        String::from_utf8(bytes.to_vec())
            .unwrap()
            .parse::<u64>()
            .unwrap_or(0)
    }

    fn write(cache_full_path: &str, map: &GoogleCacheBTreeMap) -> Result<(), SkkError> {
        let mut file = File::create(cache_full_path)?;
        let serialized_map = Self::serialize(map)?;
        let mut hasher = Sha1::new();
        hasher.update(&serialized_map);
        let digest: [u8; SHA1SUM_LENGTH] = hasher.finalize().as_slice().try_into().unwrap();
        file.write_all(&digest)?;
        file.write_all(&serialized_map)?;
        Ok(())
    }
}

#[cfg(test)]
mod test_unix {
    use crate::skk::test_unix::{read_jisyo_entries_no_encoding_conversion, setup, Path};
    use crate::skk::yaskkserv2::{GoogleCache, GoogleCacheBTreeMap};
    use crate::skk::Encoding;

    const LOOP: usize = 50;

    fn get_huge_cache_b_tree_map() -> GoogleCacheBTreeMap {
        let jisyo_entries = read_jisyo_entries_no_encoding_conversion(
            &Path::get_full_path_yaskkserv2_jisyo(Encoding::Utf8),
        );
        let mut map = GoogleCacheBTreeMap::new();
        for entry in jisyo_entries {
            if let Some(space_find) = twoway::find_bytes(&entry, b" ") {
                const SPACE_LENGTH: usize = 1;
                const SLASH_LENGTH: usize = 1;
                const LF_LENGTH: usize = 1;
                let midashi = &entry[..space_find];
                let candidates = &entry[space_find + SPACE_LENGTH + SLASH_LENGTH
                    ..entry.len() - SLASH_LENGTH - LF_LENGTH];
                let splited_candidates = candidates
                    .split(|v| *v == b'/')
                    .map(&<[u8]>::to_vec)
                    .collect::<Vec<Vec<u8>>>();
                map.insert(midashi.to_vec(), splited_candidates);
            }
        }
        map
    }

    #[test]
    fn cache_read_empty_test() {
        let name = "cache_read_empty_test";
        setup::setup_and_wait(name);
        let cache_full_path = Path::get_full_path(&format!("{name}.cache"));
        GoogleCache::write(&cache_full_path, &GoogleCacheBTreeMap::new()).unwrap();
        let bench = std::time::Instant::now();
        for _ in 0..LOOP {
            GoogleCache::read(&cache_full_path).unwrap();
        }
        println!(
            "bench cache_read_empty_test {}ms.",
            bench.elapsed().as_millis() / LOOP as u128
        );
        setup::exit();
    }

    #[test]
    fn cache_read_huge_test() {
        let name = "cache_read_huge_test";
        setup::setup_and_wait(name);
        let cache_full_path = Path::get_full_path(&format!("{name}.cache"));
        GoogleCache::write(&cache_full_path, &get_huge_cache_b_tree_map()).unwrap();
        let bench = std::time::Instant::now();
        for _ in 0..LOOP {
            GoogleCache::read(&cache_full_path).unwrap();
        }
        println!(
            "bench cache_read_huge_test {}ms.",
            bench.elapsed().as_millis() / LOOP as u128
        );
        setup::exit();
    }

    #[test]
    fn cache_write_empty_test() {
        let name = "cache_write_empty_test";
        setup::setup_and_wait(name);
        let cache_full_path = Path::get_full_path(&format!("{name}.cache"));
        let map = GoogleCacheBTreeMap::new();
        let bench = std::time::Instant::now();
        for _ in 0..LOOP {
            GoogleCache::write(&cache_full_path, &map).unwrap();
        }
        println!(
            "bench cache_write_empty_test {}ms.",
            bench.elapsed().as_millis() / LOOP as u128
        );
        setup::exit();
    }

    #[test]
    fn cache_write_huge_test() {
        let name = "cache_write_huge_test";
        setup::setup_and_wait(name);
        let cache_full_path = Path::get_full_path(&format!("{name}.cache"));
        let map = get_huge_cache_b_tree_map();
        let bench = std::time::Instant::now();
        for _ in 0..LOOP {
            GoogleCache::write(&cache_full_path, &map).unwrap();
        }
        println!(
            "bench cache_write_huge_test {}ms.",
            bench.elapsed().as_millis() / LOOP as u128
        );
        setup::exit();
    }
}
