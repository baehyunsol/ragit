use crate::INDEX_DIR_NAME;
use crate::chunk::{Chunk, CHUNK_DIR_NAME};
use crate::error::Error;
use crate::index::{FILE_INDEX_DIR_NAME, IMAGE_DIR_NAME, Index};
use lazy_static::lazy_static;
use ragit_fs::{
    WriteMode,
    exists,
    extension,
    file_name,
    file_size,
    get_relative_path,
    is_dir,
    join,
    join3,
    join4,
    read_bytes,
    read_bytes_offset,
    read_dir,
    set_extension,
    write_string,
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::str::FromStr;

/// Each chunk, image and file has uid.
///
/// Uid is a 256 bit hash value, generated by sha3-256 hash function and some postprocessing.
/// The most convenient way for users to deal with uid is using `uid_query` function. The user
/// inputs a hex representation of a uid, or a prefix of it, and the function returns
/// matched uids.
///
/// The first 192 bits (128 of `high` + 64 of `low`) are from the hash function, and
/// the remaining bits are for metadata.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Uid {
    pub(crate) high: u128,
    pub(crate) low: u128,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum UidType {
    Chunk,
    Image,
    File,
    Group,
}

lazy_static! {
    // full or prefix
    static ref UID_RE: Regex = Regex::new(r"^([0-9a-z]{1,64})$").unwrap();
}

// `Vec<Uid>` has multiple serialization formats, though only 1 is implemented now.
// Format 1, naive format: store hex representations of uids, using newline character as a delimiter.
// Format 2, compact format: TODO
pub fn load_from_file(path: &str) -> Result<Vec<Uid>, Error> {
    let bytes = read_bytes(path)?;

    match bytes.get(0) {
        Some((b'a'..=b'f') | (b'0'..=b'9')) => match String::from_utf8(bytes) {
            Ok(s) => {
                let mut result = vec![];

                for line in s.lines() {
                    result.push(line.parse::<Uid>()?);
                }

                Ok(result)
            },
            Err(_) => Err(Error::CorruptedFile(path.to_string())),
        },
        Some(_) => Err(Error::CorruptedFile(path.to_string())),
        None => Ok(vec![]),
    }
}

// For now, it only supports naive format.
pub fn save_to_file(
    path: &str,
    uids: &[Uid],
) -> Result<(), Error> {
    Ok(write_string(
        path,
        &uids.iter().map(|uid| uid.to_string()).collect::<Vec<_>>().join("\n"),
        WriteMode::CreateOrTruncate,
    )?)
}

impl Uid {
    const METADATA_MASK: u128 = 0xffff_ffff_ffff_ffff_0000_0000_0000_0000;
    const CHUNK_TYPE: u128 = (0x1 << 32);
    const IMAGE_TYPE: u128 = (0x2 << 32);
    const FILE_TYPE: u128 = (0x3 << 32);
    const GROUP_TYPE: u128 = (0x4 << 32);

    pub(crate) fn dummy() -> Self {
        Uid {
            high: 0,
            low: 0,
        }
    }

    pub fn new_chunk(chunk: &Chunk) -> Self {
        let mut hasher = Sha3_256::new();
        hasher.update(format!("{}{}{}{}", chunk.source.hash_str(), chunk.title, chunk.summary, chunk.data).as_bytes());
        let mut result = format!("{:064x}", hasher.finalize()).parse::<Uid>().unwrap();
        result.low &= Uid::METADATA_MASK;
        result.low |= Uid::CHUNK_TYPE;
        result.low |= (chunk.data.len() as u128) & 0xffff_ffff;
        result
    }

    pub fn new_image(bytes: &[u8]) -> Self {
        let mut hasher = Sha3_256::new();
        hasher.update(bytes);
        let mut result = format!("{:064x}", hasher.finalize()).parse::<Uid>().unwrap();
        result.low &= Uid::METADATA_MASK;
        result.low |= Uid::IMAGE_TYPE;
        result.low |= (bytes.len() as u128) & 0xffff_ffff;
        result
    }

    pub fn new_file(root_dir: &str, path: &str) -> Result<Self, Error> {
        let size = file_size(path)?;
        let rel_path = get_relative_path(&root_dir.to_string(), &path.to_string())?;
        let mut file_path_hasher = Sha3_256::new();
        file_path_hasher.update(rel_path.as_bytes());
        let file_path_uid = format!("{:064x}", file_path_hasher.finalize()).parse::<Uid>().unwrap();
        let mut file_content_hasher = Sha3_256::new();

        if size < 32 * 1024 * 1024 {
            let bytes = read_bytes(path)?;
            file_content_hasher.update(&bytes);
        }

        else {
            let block_size = 32 * 1024 * 1024;
            let mut offset = 0;

            loop {
                let bytes = read_bytes_offset(path, offset, offset + block_size)?;
                file_content_hasher.update(&bytes);
                offset += block_size;

                if offset >= size {
                    break;
                }
            }
        }

        let mut result = format!("{:064x}", file_content_hasher.finalize()).parse::<Uid>().unwrap();
        result ^= file_path_uid;
        result.low &= Uid::METADATA_MASK;
        result.low |= Uid::FILE_TYPE;
        result.low |= (size as u128) & 0xffff_ffff;
        Ok(result)
    }

    pub fn new_group(uids: &[Uid]) -> Self {
        let mut result = Uid::dummy();
        let mut child_count = 0;

        for uid in uids.iter() {
            result ^= *uid;

            match uid.get_uid_type() {
                Ok(UidType::Group) => { child_count += uid.get_data_size(); },
                _ => { child_count += 1; },
            }
        }

        result.low &= Uid::METADATA_MASK;
        result.low |= Uid::GROUP_TYPE;
        result.low |= (child_count as u128) & 0xffff_ffff;
        result
    }

    // TODO: this function has to be tested
    pub fn update_file_uid(mut old: Uid, old_path: &str, new_path: &str) -> Self {
        let mut old_path_hasher = Sha3_256::new();
        old_path_hasher.update(old_path.as_bytes());
        let mut old_path_uid = format!("{:064x}", old_path_hasher.finalize()).parse::<Uid>().unwrap();
        old_path_uid.low &= Uid::METADATA_MASK;
        let mut new_path_hasher = Sha3_256::new();
        new_path_hasher.update(new_path.as_bytes());
        let mut new_path_uid = format!("{:064x}", new_path_hasher.finalize()).parse::<Uid>().unwrap();
        new_path_uid.low &= Uid::METADATA_MASK;

        old ^= old_path_uid;
        old ^= new_path_uid;
        old
    }

    pub(crate) fn from_prefix_and_suffix(prefix: &str, suffix: &str) -> Result<Self, Error> {
        if prefix.len() != 2 || suffix.len() != 62 {
            Err(Error::InvalidUid(format!("{prefix}{suffix}")))
        }

        else {
            match (suffix.get(0..30), suffix.get(30..)) {
                (Some(high_suff), Some(low)) => match (
                    u128::from_str_radix(&format!("{prefix}{high_suff}"), 16),
                    u128::from_str_radix(low, 16),
                ) {
                    (Ok(high), Ok(low)) => Ok(Uid { high, low }),
                    _ => Err(Error::InvalidUid(format!("{prefix}{suffix}"))),
                },
                _ => Err(Error::InvalidUid(format!("{prefix}{suffix}"))),
            }
        }
    }

    pub(crate) fn get_prefix(&self) -> String {
        format!("{:02x}", self.high >> 120)
    }

    pub(crate) fn get_suffix(&self) -> String {
        format!("{:030x}{:032x}", self.high & 0xff_ffff_ffff_ffff_ffff_ffff_ffff_ffff, self.low)
    }

    pub fn get_short_name(&self) -> String {
        format!("{:08x}", self.high >> 96)
    }

    pub(crate) fn get_uid_type(&self) -> Result<UidType, Error> {
        let field = ((self.low >> 32) & 0xf) << 32;

        match field {
            Uid::CHUNK_TYPE => Ok(UidType::Chunk),
            Uid::IMAGE_TYPE => Ok(UidType::Image),
            Uid::FILE_TYPE => Ok(UidType::File),
            Uid::GROUP_TYPE => Ok(UidType::Group),
            _ => Err(Error::InvalidUid(self.to_string())),
        }
    }

    pub(crate) fn get_data_size(&self) -> usize {
        (self.low & 0xffff_ffff) as usize
    }
}

impl fmt::Display for Uid {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(fmt, "{:032x}{:032x}", self.high, self.low)
    }
}

impl FromStr for Uid {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        if s.len() != 64 {
            Err(Error::InvalidUid(s.to_string()))
        }

        else {
            match (s.get(0..32), s.get(32..)) {
                (Some(high), Some(low)) => match (
                    u128::from_str_radix(high, 16),
                    u128::from_str_radix(low, 16),
                ) {
                    (Ok(high), Ok(low)) => Ok(Uid { high, low }),
                    _ => Err(Error::InvalidUid(s.to_string())),
                },
                _ => Err(Error::InvalidUid(s.to_string())),
            }
        }
    }
}

impl std::ops::BitXor for Uid {
    type Output = Self;

    fn bitxor(self, rhs: Self) -> Self {
        Uid {
            low: self.low ^ rhs.low,
            high: self.high ^ rhs.high,
        }
    }
}

impl std::ops::BitXorAssign for Uid {
    fn bitxor_assign(&mut self, rhs: Self) {
        self.low ^= rhs.low;
        self.high ^= rhs.high;
    }
}

impl Index {
    /// The result is sorted by uid.
    /// Sorting 1) makes the result deterministic and 2) some functions rely on this behavior.
    pub fn get_all_chunk_uids(&self) -> Result<Vec<Uid>, Error> {
        let mut result = vec![];

        for internal in read_dir(&join3(&self.root_dir, &INDEX_DIR_NAME, &CHUNK_DIR_NAME)?, false)? {
            let prefix = file_name(&internal)?;

            if !is_dir(&internal) {
                continue;
            }

            for chunk_file in read_dir(&internal, false)? {
                if extension(&chunk_file).unwrap_or(None).unwrap_or(String::new()) == "chunk" {
                    result.push(Uid::from_prefix_and_suffix(&prefix, &file_name(&chunk_file)?)?);
                }
            }
        }

        result.sort();
        Ok(result)
    }

    /// The result is sorted by uid.
    /// Sorting 1) makes the result deterministic and 2) some functions rely on this behavior.
    pub fn get_all_image_uids(&self) -> Result<Vec<Uid>, Error> {
        let mut result = vec![];

        for internal in read_dir(&join3(&self.root_dir, &INDEX_DIR_NAME, &IMAGE_DIR_NAME)?, false)? {
            let prefix = file_name(&internal)?;

            if !is_dir(&internal) {
                continue;
            }

            for image_file in read_dir(&internal, false)? {
                if extension(&image_file).unwrap_or(None).unwrap_or(String::new()) == "png" {
                    result.push(Uid::from_prefix_and_suffix(&prefix, &file_name(&image_file)?)?);
                }
            }
        }

        result.sort();
        Ok(result)
    }

    /// The result is sorted by uid.
    /// Sorting 1) makes the result deterministic and 2) some functions rely on this behavior.
    pub fn get_all_file_uids(&self) -> Vec<Uid> {
        let mut result: Vec<Uid> = self.processed_files.values().map(|uid| *uid).collect();
        result.sort();
        result
    }

    pub fn uid_query(&self, qs: &[String], config: UidQueryConfig) -> Result<UidQueryResult, Error> {
        let mut chunks_set = HashSet::new();
        let mut images_set = HashSet::new();
        let mut processed_files_map = HashMap::new();
        let mut staged_files_set = HashSet::new();

        for q in qs.iter() {
            let curr = self.uid_query_unit(q, config)?;

            for chunk in curr.chunks.iter() {
                chunks_set.insert(*chunk);
            }

            for image in curr.images.iter() {
                images_set.insert(*image);
            }

            for (path, uid) in curr.processed_files.iter() {
                processed_files_map.insert(*uid, path.to_string());
            }

            for staged_file in curr.staged_files.iter() {
                staged_files_set.insert(staged_file.to_string());
            }
        }

        let mut chunks = chunks_set.into_iter().collect::<Vec<_>>();
        let mut images = images_set.into_iter().collect::<Vec<_>>();
        let mut processed_files = processed_files_map.into_iter().map(|(uid, path)| (path, uid)).collect::<Vec<_>>();
        let mut staged_files = staged_files_set.into_iter().collect::<Vec<_>>();

        // The result has to be deterministic
        chunks.sort();
        images.sort();
        processed_files.sort_by_key(|(_, uid)| *uid);
        staged_files.sort();

        Ok(UidQueryResult {
            chunks,
            images,
            processed_files,
            staged_files,
        })
    }

    fn uid_query_unit(&self, q: &str, config: UidQueryConfig) -> Result<UidQueryResult, Error> {
        if q.is_empty() {
            return Ok(UidQueryResult::empty());
        }

        let mut chunks = vec![];
        let mut images = vec![];
        let mut staged_files = vec![];

        // below 2 are for processed files
        let mut file_uids = vec![];
        let mut file_paths = vec![];

        if UID_RE.is_match(q) {
            if q.len() == 1 {
                if config.search_chunk {
                    for chunk_dir in read_dir(&join3(
                        &self.root_dir,
                        INDEX_DIR_NAME,
                        CHUNK_DIR_NAME,
                    )?, false).unwrap_or(vec![]) {
                        let chunk_prefix = file_name(&chunk_dir)?;

                        if chunk_prefix.starts_with(q) {
                            for chunk_file in read_dir(&chunk_dir, false)? {
                                if extension(&chunk_file)?.unwrap_or(String::new()) != "chunk" {
                                    continue;
                                }

                                chunks.push(Uid::from_prefix_and_suffix(&chunk_prefix, &file_name(&chunk_file)?)?);
                            }
                        }
                    }
                }

                if config.search_file_uid {
                    for file_index_dir in read_dir(&join3(
                        &self.root_dir,
                        INDEX_DIR_NAME,
                        FILE_INDEX_DIR_NAME,
                    )?, false).unwrap_or(vec![]) {
                        let file_index_prefix = file_name(&file_index_dir)?;

                        if file_index_prefix.starts_with(q) {
                            for file_index in read_dir(&file_index_dir, false)? {
                                file_uids.push(Uid::from_prefix_and_suffix(&file_index_prefix, &file_name(&file_index)?)?);
                            }
                        }
                    }
                }

                if config.search_image {
                    for image_dir in read_dir(&join3(
                        &self.root_dir,
                        INDEX_DIR_NAME,
                        IMAGE_DIR_NAME,
                    )?, false).unwrap_or(vec![]) {
                        let image_prefix = file_name(&image_dir)?;

                        if image_prefix.starts_with(q) {
                            for image_file in read_dir(&image_dir, false)? {
                                if extension(&image_file)?.unwrap_or(String::new()) != "png" {
                                    continue;
                                }

                                images.push(Uid::from_prefix_and_suffix(&image_prefix, &file_name(&image_file)?)?);
                            }
                        }
                    }
                }
            }

            else if q.len() == 2 {
                if config.search_chunk {
                    for chunk_file in read_dir(&join4(
                        &self.root_dir,
                        INDEX_DIR_NAME,
                        CHUNK_DIR_NAME,
                        q,
                    )?, false).unwrap_or(vec![]) {
                        if extension(&chunk_file)?.unwrap_or(String::new()) != "chunk" {
                            continue;
                        }

                        chunks.push(Uid::from_prefix_and_suffix(q, &file_name(&chunk_file)?)?);
                    }
                }

                if config.search_file_uid {
                    for file_index in read_dir(&join4(
                        &self.root_dir,
                        INDEX_DIR_NAME,
                        FILE_INDEX_DIR_NAME,
                        q,
                    )?, false).unwrap_or(vec![]) {
                        file_uids.push(Uid::from_prefix_and_suffix(q, &file_name(&file_index)?)?);
                    }
                }

                if config.search_image {
                    for image_file in read_dir(&join4(
                        &self.root_dir,
                        INDEX_DIR_NAME,
                        IMAGE_DIR_NAME,
                        q,
                    )?, false).unwrap_or(vec![]) {
                        if extension(&image_file)?.unwrap_or(String::new()) != "png" {
                            continue;
                        }

                        images.push(Uid::from_prefix_and_suffix(q, &file_name(&image_file)?)?);
                    }
                }
            }

            else {
                let prefix = q.get(0..2).unwrap().to_string();
                let suffix = q.get(2..).unwrap().to_string();

                if config.search_chunk {
                    if q.len() == 64 {
                        let chunk_at = join(
                            &join3(
                                &self.root_dir,
                                INDEX_DIR_NAME,
                                CHUNK_DIR_NAME,
                            )?,
                            &join(
                                &prefix,
                                &set_extension(
                                    &suffix,
                                    "chunk",
                                )?,
                            )?,
                        )?;

                        if exists(&chunk_at) {
                            chunks.push(q.parse::<Uid>()?);
                        }
                    }

                    else {
                        for chunk_file in read_dir(&join4(
                            &self.root_dir,
                            INDEX_DIR_NAME,
                            CHUNK_DIR_NAME,
                            &prefix,
                        )?, false).unwrap_or(vec![]) {
                            if extension(&chunk_file)?.unwrap_or(String::new()) != "chunk" {
                                continue;
                            }

                            let chunk_file = file_name(&chunk_file)?;

                            if chunk_file.starts_with(&suffix) {
                                chunks.push(Uid::from_prefix_and_suffix(&prefix, &chunk_file)?);
                            }
                        }
                    }
                }

                if config.search_file_uid {
                    if q.len() == 64 {
                        let file_index = join(
                            &join3(
                                &self.root_dir,
                                INDEX_DIR_NAME,
                                FILE_INDEX_DIR_NAME,
                            )?,
                            &join(
                                &prefix,
                                &suffix,
                            )?,
                        )?;

                        if exists(&file_index) {
                            file_uids.push(q.parse::<Uid>()?);
                        }
                    }

                    else {
                        for file_index in read_dir(&join4(
                            &self.root_dir,
                            INDEX_DIR_NAME,
                            FILE_INDEX_DIR_NAME,
                            &prefix,
                        )?, false).unwrap_or(vec![]) {
                            let file_index = file_name(&file_index)?;

                            if file_index.starts_with(&suffix) {
                                file_uids.push(Uid::from_prefix_and_suffix(&prefix, &file_index)?);
                            }
                        }
                    }
                }

                if config.search_image {
                    if q.len() == 64 {
                        let image_at = join(
                            &join3(
                                &self.root_dir,
                                INDEX_DIR_NAME,
                                IMAGE_DIR_NAME,
                            )?,
                            &join(
                                &prefix,
                                &set_extension(
                                    &suffix,
                                    "png",
                                )?,
                            )?,
                        )?;

                        if exists(&image_at) {
                            images.push(q.parse::<Uid>()?);
                        }
                    }

                    else {
                        for image_file in read_dir(&join4(
                            &self.root_dir,
                            INDEX_DIR_NAME,
                            IMAGE_DIR_NAME,
                            &prefix,
                        )?, false).unwrap_or(vec![]) {
                            if extension(&image_file)?.unwrap_or(String::new()) != "png" {
                                continue;
                            }

                            let image_file = file_name(&image_file)?;

                            if image_file.starts_with(&suffix) {
                                images.push(Uid::from_prefix_and_suffix(&prefix, &image_file)?);
                            }
                        }
                    }
                }
            }
        }

        if config.search_file_path {
            if let Ok(mut rel_path) = get_relative_path(&self.root_dir, q) {
                // 1. It tries to exact-match a processed file.
                if self.processed_files.contains_key(&rel_path) {
                    file_paths.push(rel_path.to_string());
                }

                // 2. It tries to exact-match a staged file.
                //    In some cases, a file can be both processed and staged at the
                //    same time. In that case, it has to choose the processed file.
                else if config.search_staged_file && self.staged_files.contains(&rel_path) {
                    staged_files.push(rel_path);
                }

                // 3. It assumes that `rel_path` is a directory and tries to
                //    find files in the directory.
                else {
                    if !rel_path.ends_with("/") && !rel_path.is_empty() {
                        rel_path = format!("{rel_path}/");
                    }

                    for path in self.processed_files.keys() {
                        if path.starts_with(&rel_path) {
                            file_paths.push(path.to_string());
                        }
                    }

                    if config.search_staged_file {
                        for staged_file in self.staged_files.iter() {
                            if staged_file.starts_with(&rel_path) {
                                staged_files.push(staged_file.to_string());
                            }
                        }
                    }
                }
            }
        }

        let mut processed_files = HashSet::with_capacity(file_paths.len() + file_uids.len());
        let processed_files_rev: HashMap<_, _> = self.processed_files.iter().map(
            |(file, uid)| (*uid, file.to_string())
        ).collect();

        for path in file_paths.iter() {
            processed_files.insert((path.to_string(), *self.processed_files.get(path).unwrap()));
        }

        for uid in file_uids.iter() {
            processed_files.insert((processed_files_rev.get(uid).unwrap().to_string(), *uid));
        }

        let processed_files: Vec<(String, Uid)> = processed_files.into_iter().collect();

        Ok(UidQueryResult {
            chunks,
            images,
            processed_files,
            staged_files,
        })
    }
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct UidQueryConfig {
    pub search_chunk: bool,
    pub search_image: bool,
    pub search_file_path: bool,
    pub search_file_uid: bool,

    /// It searches staged files when both `search_file_path` and `search_staged_file` are set.
    pub search_staged_file: bool,
}

impl UidQueryConfig {
    pub fn new() -> Self {
        UidQueryConfig {
            search_chunk: true,
            search_image: true,
            search_file_path: true,
            search_file_uid: true,
            search_staged_file: true,
        }
    }

    pub fn file_or_chunk(mut self) -> Self {
        self.search_chunk = true;
        self.search_file_path = true;
        self.search_file_uid = true;
        self
    }

    pub fn file_only(mut self) -> Self {
        self.search_chunk = false;
        self.search_image = false;
        self.search_file_path = true;
        self.search_file_uid = true;
        self
    }

    pub fn no_staged_file(mut self) -> Self {
        self.search_staged_file = false;
        self
    }
}

#[derive(Clone, Debug)]
pub struct UidQueryResult {
    pub chunks: Vec<Uid>,
    pub images: Vec<Uid>,
    pub processed_files: Vec<(String, Uid)>,
    pub staged_files: Vec<String>,
}

impl UidQueryResult {
    fn empty() -> Self {
        UidQueryResult {
            chunks: vec![],
            images: vec![],
            processed_files: vec![],
            staged_files: vec![],
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn has_multiple_matches(&self) -> bool {
        self.len() > 1
    }

    pub fn len(&self) -> usize {
        self.chunks.len() + self.images.len() + self.processed_files.len() + self.staged_files.len()
    }

    pub fn get_chunk_uids(&self) -> Vec<Uid> {
        self.chunks.clone()
    }

    pub fn get_image_uids(&self) -> Vec<Uid> {
        self.images.clone()
    }

    pub fn get_file_uids(&self) -> Vec<Uid> {
        self.processed_files.iter().map(|(_, uid)| *uid).collect()
    }

    pub fn get_processed_files(&self) -> Vec<(String, Uid)> {
        self.processed_files.clone()
    }

    pub fn get_staged_files(&self) -> Vec<String> {
        self.staged_files.clone()
    }

    /// It returns `Some` iff there's only 1 match.
    pub fn get_processed_file(&self) -> Option<(String, Uid)> {
        if self.processed_files.len() == 1 {
            Some(self.processed_files[0].clone())
        }

        else {
            None
        }
    }

    /// It returns `Some` iff there's only 1 match.
    pub fn get_staged_file(&self) -> Option<String> {
        if self.staged_files.len() == 1 {
            Some(self.staged_files[0].clone())
        }

        else {
            None
        }
    }

    /// It returns `Some` iff there's only 1 match.
    pub fn get_chunk_uid(&self) -> Option<Uid> {
        if self.chunks.len() == 1 {
            Some(self.chunks[0])
        }

        else {
            None
        }
    }

    /// It returns `Some` iff there's only 1 match.
    pub fn get_image_uid(&self) -> Option<Uid> {
        if self.images.len() == 1 {
            Some(self.images[0])
        }

        else {
            None
        }
    }
}
