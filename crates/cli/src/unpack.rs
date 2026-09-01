use anyhow::Result;
use std::path::{Component, Path};

pub fn unpack(data: &[u8], dest: &Path) -> Result<u64> {
    std::fs::create_dir_all(dest)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dest, std::fs::Permissions::from_mode(0o700))?;
    }

    let tar_data = zstd::decode_all(data)?;

    let mut archive = tar::Archive::new(std::io::Cursor::new(tar_data));
    let mut count = 0u64;

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();

        let is_safe = path
            .components()
            .all(|component| matches!(component, Component::Normal(_)));
        if !is_safe {
            tracing::warn!("Skipping unsafe path: {:?}", path);
            continue;
        }

        let dest_path = dest.join(&path);
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        entry.unpack(&dest_path)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dest_path, std::fs::Permissions::from_mode(0o600))?;
        }
        count += 1;
    }

    Ok(count)
}
