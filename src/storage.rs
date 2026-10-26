use std::{
    io::{self, Write},
    path::Path,
};

// 整个事务在惰性阻塞任务中完成；取消等待不会中断替换过程或遗留半写入文件。
pub async fn write_atomic(path: &Path, bytes: Vec<u8>) -> io::Result<()> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut temporary = tempfile::Builder::new()
            .prefix(".mini-deck-")
            .suffix(".tmp")
            .tempfile_in(parent)?;
        temporary.write_all(&bytes)?;
        temporary.as_file().sync_all()?;
        // persist 在同一文件系统内原子替换，失败时 RAII 只清理本次创建的文件。
        temporary.persist(&path).map_err(|e| e.error)?;
        Ok(())
    })
    .await
    .map_err(io::Error::other)?
}
