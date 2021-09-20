use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::FileExt;
use std::sync::Arc;

use crate::LTPError;

const PAGE_SIZE: usize = 4 * 1024;
const LINE_DELIMITER: u8 = b'\n';

#[derive(Clone)]
pub(super) struct Storage {
    path: String,
    index: Index,
}

impl Storage {
    pub(super) fn init(path: &str) -> Result<Self, LTPError> {
        if !File::open(path)?.metadata()?.is_file() {
            return Err(LTPError::GenericFailure("File does not exist".into()));
        }

        Ok(Storage {
            path: path.to_string(),
            index: Index::index(path)?,
        })
    }

    pub(super) async fn read(&self, line_number: u64) -> Option<String> {
        let (indexed_line_number, offset) = self.index.resolve(line_number)?;
        let path = self.path.clone();
        tokio::task::spawn_blocking(move || {
            let file = File::open(path).ok()?;
            let line_offset = Self::find_line_offset(&file, offset, line_number - indexed_line_number)?;
            Self::read_line(&file, line_offset)
        })
        .await
        .ok()?
    }

    fn find_line_offset(file: &File, from_offset: u64, line_number: u64) -> Option<u64> {
        let mut offset = from_offset;
        let mut lines_to_skip = line_number;
        while lines_to_skip > 0 {
            let mut bytes = vec![0u8; PAGE_SIZE];
            let bytes_read = file.read_at(&mut bytes, offset).ok()?;
            if bytes_read == 0 {
                return None;
            }

            let mut index = 0;
            while index < bytes_read && lines_to_skip > 0 {
                if bytes[index] == LINE_DELIMITER {
                    lines_to_skip -= 1;
                }
                index += 1;
                offset += 1;
            }
        }
        Some(offset)
    }

    fn read_line(file: &File, offset: u64) -> Option<String> {
        let mut index = 0;
        let mut bytes = Vec::new();
        loop {
            bytes.extend_from_slice(&[0u8; PAGE_SIZE]);
            let bytes_read = file.read_at(&mut bytes, offset).ok()?;
            if bytes_read == 0 {
                return None;
            }

            while index < bytes_read {
                if bytes[index] == LINE_DELIMITER {
                    bytes.truncate(index);
                    return String::from_utf8(bytes).ok();
                }
                index += 1;
            }
        }
    }
}

// 128 bytes (u64 + u64) * 500_000 ~= 64MB
const MAX_NUMBER_OF_INDEXED_LINES: u64 = 500_000;

/// Size limited index, mapping line numbers to their corresponding byte offsets on disk.
#[derive(Clone)]
struct Index {
    entries: Arc<BTreeMap<u64, u64>>,
}

impl Index {
    fn index(path: &str) -> Result<Self, LTPError> {
        let number_of_lines = BufReader::new(File::open(path)?).lines().count();
        let nth = u64::max(
            1,
            u64::try_from(number_of_lines).expect("Unable to convert") / MAX_NUMBER_OF_INDEXED_LINES,
        );

        let mut line_number = 0;
        let mut offset = 0;
        let mut entries = BTreeMap::new();
        for line in BufReader::new(File::open(path)?).lines() {
            let line = line?;
            if line_number % nth == 0 {
                entries.insert(line_number + 1, offset);
            }
            line_number += 1;
            offset += u64::try_from(line.len()).expect("Unable to convert") + 1;
        }

        Ok(Index {
            entries: Arc::new(entries),
        })
    }

    /// Given line number, returns a tuple of a closest (less than or equal) indexed line number and its offset.
    fn resolve(&self, line_number: u64) -> Option<(u64, u64)> {
        self.entries
            .range(..=line_number)
            .next_back()
            .map(|(line_number, offset)| (*line_number, *offset))
    }
}
