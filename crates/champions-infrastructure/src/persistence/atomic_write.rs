use std::fs;
use std::io::Write;
use std::path::Path;

pub fn atomic_write(target: &Path, content: &[u8]) -> std::io::Result<()> {
    let tmp = target.with_extension("tmp");

    let mut file = fs::File::create(&tmp)?;
    file.write_all(content)?;
    file.sync_all()?;
    drop(file);

    let bak = target.with_extension("bak");
    if bak.exists() {
        if let Err(error) = fs::remove_file(&bak) {
            let _ = fs::remove_file(&tmp);
            return Err(error);
        }
    }

    if target.exists() {
        if let Err(error) = fs::rename(target, &bak) {
            let _ = fs::remove_file(&tmp);
            return Err(error);
        }
    }

    if let Err(e) = fs::rename(&tmp, target) {
        let _ = fs::remove_file(&tmp);
        if bak.exists() {
            let _ = fs::rename(&bak, target);
        }
        return Err(e);
    }

    let _ = fs::remove_file(&bak);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_write_creates_file() {
        let dir = std::env::temp_dir().join("champions_test_atomic");
        let _ = fs::create_dir_all(&dir);
        let target = dir.join("test_file.json");

        atomic_write(&target, b"hello").unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "hello");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_atomic_write_overwrites_existing() {
        let dir = std::env::temp_dir().join("champions_test_atomic2");
        let _ = fs::create_dir_all(&dir);
        let target = dir.join("test_file.json");

        fs::write(&target, "old").unwrap();
        atomic_write(&target, b"new").unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "new");

        assert!(!target.with_extension("bak").exists());
        assert!(!target.with_extension("tmp").exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_atomic_write_overwrites_even_with_stale_backup() {
        let dir = std::env::temp_dir().join("champions_test_atomic3");
        let _ = fs::create_dir_all(&dir);
        let target = dir.join("test_file.json");
        let bak = target.with_extension("bak");

        fs::write(&target, "old").unwrap();
        fs::write(&bak, "stale").unwrap();

        atomic_write(&target, b"new").unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "new");
        assert!(!bak.exists());
        assert!(!target.with_extension("tmp").exists());

        let _ = fs::remove_dir_all(&dir);
    }
}
