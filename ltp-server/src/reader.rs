use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::FileExt;
use std::sync::Arc;

use tokio::task::spawn_blocking;

use crate::LTPError;

#[derive(Clone)]
pub(super) struct Reader {
    path: String,
    index: Index,
}

impl Reader {
    pub(super) fn new(path: &str) -> Result<Self, LTPError> {
        Ok(Reader {
            path: path.to_string(),
            index: Index::index(path)?,
        })
    }

    pub(super) async fn read(&self, line_number: u64) -> Option<String> {
        if line_number == 0 {
            return None;
        }

        let (from, to) = self.index.resolve(line_number)?;
        let path = self.path.clone();
        // TODO get our hands on io_uring when it's there!
        spawn_blocking(move || {
            let file = File::open(path).ok()?;
            let mut bytes = vec![0u8; to - from];
            let read_bytes = file
                .read_at(&mut bytes, u64::try_from(from).expect("Unable to convert"))
                .ok()?;
            assert_eq!(read_bytes, to - from);
            String::from_utf8(bytes).ok()
        })
        .await
        .ok()?
    }
}

#[derive(Clone)]
struct Index {
    index: Arc<BTreeMap<u64, (usize, usize)>>,
}

impl Index {
    // TODO: consider indexing every nth line
    fn index(path: &str) -> Result<Self, LTPError> {
        let file = File::open(path)?;
        let mut line_number = 1;
        let mut offset = 0;
        let mut index = BTreeMap::new();
        for line in BufReader::new(file).lines() {
            let line = line?;
            index.insert(line_number, (offset, offset + line.len()));
            line_number += 1;
            offset += line.len() + 1;
        }
        Ok(Index { index: Arc::new(index) })
    }

    fn resolve(&self, line_number: u64) -> Option<(usize, usize)> {
        self.index.get(&line_number).map(|(from, to)| (*from, *to))
    }
}
