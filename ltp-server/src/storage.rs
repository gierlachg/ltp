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
        assert!(line_number > 0);
        let line_number = line_number - 1;
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
        let mut bytes = vec![0u8; PAGE_SIZE];
        while lines_to_skip > 0 {
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
            bytes.fill(0u8);
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

const MAX_NUMBER_OF_INDEXED_LINES: usize = 1024 * 1024;

/// Size limited (64 bytes * 1024 * 1024 = 64MB) index, mapping line numbers to their corresponding byte offsets on
/// disk.
// TODO: consider using u32 for smaller files
#[derive(Clone)]
struct Index {
    nth: u64,
    entries: Arc<Vec<u64>>,
}

impl Index {
    fn index(path: &str) -> Result<Self, LTPError> {
        let number_of_lines = BufReader::new(File::open(path)?).lines().count();
        let nth = usize::max(1, number_of_lines / MAX_NUMBER_OF_INDEXED_LINES);

        let mut offset = 0;
        let mut entries = Vec::with_capacity(number_of_lines / nth);
        for (line_number, line) in BufReader::new(File::open(path)?).lines().enumerate() {
            let line = line?;
            if line_number % nth == 0 {
                entries.push(offset);
            }
            offset += u64::try_from(line.len()).expect("Unable to convert") + 1;
        }

        Ok(Index {
            nth: u64::try_from(nth).expect("Unable to convert"),
            entries: Arc::new(entries),
        })
    }

    /// Given line number, returns a tuple of a closest (less than or equal) indexed line number and its offset.
    fn resolve(&self, line_number: u64) -> Option<(u64, u64)> {
        let indexed_line_number = line_number - (line_number % self.nth);
        let index = usize::try_from(indexed_line_number / self.nth).expect("Unable to convert");
        self.entries.get(index).map(|offset| (indexed_line_number, *offset))
    }
}
