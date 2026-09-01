use anyhow::Result;
use std::io::Write;
use std::path::Path;
use walkdir::WalkDir;

pub struct PackResult {
    pub data: Vec<u8>,
    pub file_count: u64,
}

pub fn pack(dir: &Path) -> Result<PackResult> {
    let mut tar_data = Vec::new();
    let mut file_count = 0u64;
    {
        let mut builder = tar::Builder::new(&mut tar_data);

        for entry in WalkDir::new(dir).follow_links(false) {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let rel = path.strip_prefix(dir)?;
            let mut header = tar::Header::new_gnu();
            let metadata = std::fs::metadata(path)?;
            header.set_metadata(&metadata);
            header.set_mode(0o600);
            header.set_size(metadata.len());
            header.set_cksum();
            let mut file = std::fs::File::open(path)?;
            builder.append_data(&mut header, rel, &mut file)?;
            file_count += 1;
        }
        builder.finish()?;
    }

    let mut compressed = Vec::new();
    let mut encoder = zstd::Encoder::new(&mut compressed, 9)?;
    encoder.write_all(&tar_data)?;
    encoder.finish()?;

    Ok(PackResult {
        data: compressed,
        file_count,
    })
}
