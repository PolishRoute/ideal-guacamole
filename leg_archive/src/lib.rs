use std::path::Path;
use std::io::{Seek, SeekFrom, Read, BufRead, BufReader};
use std::ops::Range;

#[derive(Debug)]
struct ArchiveEntry {
    file_name: String,
    range: Range<u64>,
}

pub struct Archive {
    buffer: BufReader<std::fs::File>,
    files: Vec<ArchiveEntry>,
    case_sensitive: bool,
}

impl Archive {
    pub fn read(&mut self, name: &str) -> Option<Box<[u8]>> {
        let eq = if self.case_sensitive {
            str::eq
        } else {
            str::eq_ignore_ascii_case
        };

        let entry = self.files
            .iter()
            .find(|f| eq(&f.file_name, name))?;
        let Range { start, end } = entry.range.clone();
        let len = (end - start) as usize;

        self.buffer.seek(SeekFrom::Start(start)).ok()?;
        let mut buf = vec![0u8; len];
        self.buffer.read_exact(&mut buf).ok()?;
        Some(buf.into_boxed_slice())
    }
}

const ENDTABLEIDENTIFICATION: &[u8; 10] = b"LEGARCHTBL";

pub fn load(path: impl AsRef<Path>, case_sensitive: bool) -> Result<Archive, Box<dyn std::error::Error>> {
    let file = std::fs::File::open(path.as_ref())?;
    let mut reader = std::io::BufReader::new(file);
    reader.seek(SeekFrom::End(-8))?;

    let start_pos = {
        let mut x = [0u8; 8];
        reader.read_exact(&mut x)?;
        i64::from_ne_bytes(x)
    };

    reader.seek(SeekFrom::Start(start_pos as u64))?;

    let mut header = [0u8; 10];
    reader.read_exact(&mut header)?;
    if &header != ENDTABLEIDENTIFICATION {
        panic!();
    }
    let total_files = {
        let mut x = [0u8; 4];
        reader.read_exact(&mut x)?;
        i32::from_le_bytes(x)
    };

    let mut files = Vec::with_capacity(total_files as usize);
    let mut file_name = Vec::new();
    for _ in 0..total_files {
        reader.read_until(b'\0', &mut file_name)?;
        let name = std::str::from_utf8(&file_name[..file_name.len() - 1])?;

        let position = {
            let mut x = [0u8; 8];
            reader.read_exact(&mut x)?;
            i64::from_le_bytes(x)
        } as u64;

        let length = {
            let mut x = [0u8; 4];
            reader.read_exact(&mut x)?;
            i32::from_le_bytes(x)
        } as u64;

        files.push(ArchiveEntry {
            file_name: name.to_string(),
            range: position..position + length,
        });

        file_name.clear();
    }

    Ok(Archive {
        buffer: reader,
        files,
        case_sensitive,
    })
}