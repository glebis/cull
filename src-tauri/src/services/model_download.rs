use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use reqwest::header;
use serde::Serialize;
use std::fmt::Display;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelDownloadProgress {
    pub downloaded: u64,
    pub total: u64,
    pub status: String,
    pub resumable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelDownloadOutcome {
    pub downloaded: u64,
    pub resumed: bool,
}

#[derive(Clone)]
pub struct DownloadControl {
    cancel: CancellationToken,
    pause: PauseController,
}

impl Default for DownloadControl {
    fn default() -> Self {
        Self {
            cancel: CancellationToken::new(),
            pause: PauseController::default(),
        }
    }
}

impl DownloadControl {
    pub fn new(cancel: CancellationToken, pause: PauseController) -> Self {
        Self { cancel, pause }
    }

    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancel.clone()
    }

    async fn wait_until_ready(&self) -> Result<(), String> {
        if self.cancel.is_cancelled() {
            return Err("Download cancelled".to_string());
        }
        self.pause.wait_until_resumed(&self.cancel).await
    }
}

#[derive(Clone)]
pub struct PauseController {
    inner: Arc<PauseState>,
}

struct PauseState {
    paused: AtomicBool,
    notify: Notify,
}

impl Default for PauseController {
    fn default() -> Self {
        Self {
            inner: Arc::new(PauseState {
                paused: AtomicBool::new(false),
                notify: Notify::new(),
            }),
        }
    }
}

impl PauseController {
    pub fn pause(&self) {
        self.inner.paused.store(true, Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.inner.paused.store(false, Ordering::SeqCst);
        self.inner.notify.notify_waiters();
    }

    pub fn is_paused(&self) -> bool {
        self.inner.paused.load(Ordering::SeqCst)
    }

    async fn wait_until_resumed(&self, cancel: &CancellationToken) -> Result<(), String> {
        while self.is_paused() {
            tokio::select! {
                _ = self.inner.notify.notified() => {}
                _ = cancel.cancelled() => return Err("Download cancelled".to_string()),
            }
        }
        if cancel.is_cancelled() {
            return Err("Download cancelled".to_string());
        }
        Ok(())
    }
}

pub fn part_path_for(destination: &Path) -> PathBuf {
    let Some(file_name) = destination.file_name() else {
        return destination.with_extension("part");
    };
    let mut part_name = file_name.to_os_string();
    part_name.push(".part");
    destination.with_file_name(part_name)
}

#[allow(dead_code)]
pub async fn download_model_file<F>(
    client: &reqwest::Client,
    url: &str,
    destination: &Path,
    progress: F,
) -> Result<ModelDownloadOutcome, String>
where
    F: FnMut(ModelDownloadProgress),
{
    download_model_file_controlled(
        client,
        url,
        destination,
        &DownloadControl::default(),
        progress,
    )
    .await
}

pub async fn download_model_file_controlled<F>(
    client: &reqwest::Client,
    url: &str,
    destination: &Path,
    control: &DownloadControl,
    mut progress: F,
) -> Result<ModelDownloadOutcome, String>
where
    F: FnMut(ModelDownloadProgress),
{
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Model directory create error: {}", e))?;
    }

    let part_path = part_path_for(destination);
    let resume_from = fs::metadata(&part_path).map(|m| m.len()).unwrap_or(0);

    let mut request = client.get(url);
    if resume_from > 0 {
        request = request.header(header::RANGE, format!("bytes={}-", resume_from));
    }

    let response = request
        .send()
        .await
        .map_err(|e| format!("Request error: {}", e))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("Download HTTP error: {}", status));
    }

    let mut write_offset = resume_from;
    let resumed = resume_from > 0 && status == reqwest::StatusCode::PARTIAL_CONTENT;
    if resume_from > 0 && !resumed {
        let _ = fs::remove_file(&part_path);
        write_offset = 0;
    }

    let total = response
        .content_length()
        .map(|remaining| remaining + write_offset);
    write_stream_to_model_file_controlled(
        destination,
        write_offset,
        total,
        response.bytes_stream(),
        control,
        &mut progress,
    )
    .await?;

    Ok(ModelDownloadOutcome {
        downloaded: fs::metadata(destination).map(|m| m.len()).unwrap_or(0),
        resumed,
    })
}

#[allow(dead_code)]
pub async fn write_stream_to_model_file<S, E, F>(
    destination: &Path,
    resume_from: u64,
    total: Option<u64>,
    stream: S,
    progress: F,
) -> Result<u64, String>
where
    S: Stream<Item = Result<Bytes, E>> + Unpin,
    E: Display,
    F: FnMut(ModelDownloadProgress),
{
    write_stream_to_model_file_controlled(
        destination,
        resume_from,
        total,
        stream,
        &DownloadControl::default(),
        progress,
    )
    .await
}

pub async fn write_stream_to_model_file_controlled<S, E, F>(
    destination: &Path,
    resume_from: u64,
    total: Option<u64>,
    mut stream: S,
    control: &DownloadControl,
    mut progress: F,
) -> Result<u64, String>
where
    S: Stream<Item = Result<Bytes, E>> + Unpin,
    E: Display,
    F: FnMut(ModelDownloadProgress),
{
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Model directory create error: {}", e))?;
    }

    let part_path = part_path_for(destination);
    let mut options = OpenOptions::new();
    options.create(true).write(true);
    if resume_from > 0 {
        let current_len = fs::metadata(&part_path)
            .map_err(|e| format!("Partial model metadata error: {}", e))?
            .len();
        if current_len != resume_from {
            return Err(format!(
                "Partial model size changed from {} to {} before resume",
                resume_from, current_len
            ));
        }
        options.append(true);
    } else {
        options.truncate(true);
    }

    let mut file = options
        .open(&part_path)
        .map_err(|e| format!("Partial model file open error: {}", e))?;
    let mut downloaded = resume_from;
    emit_progress(&mut progress, downloaded, total, "downloading");

    while let Some(chunk) = stream.next().await {
        control.wait_until_ready().await?;
        let chunk = chunk.map_err(|e| format!("Download error: {}", e))?;
        file.write_all(&chunk)
            .map_err(|e| format!("Partial model write error: {}", e))?;
        downloaded += chunk.len() as u64;

        let first_chunk = downloaded == resume_from + chunk.len() as u64;
        if first_chunk
            || downloaded % (512 * 1024) < chunk.len() as u64
            || total == Some(downloaded)
        {
            emit_progress(&mut progress, downloaded, total, "downloading");
        }
        control.wait_until_ready().await?;
    }

    control.wait_until_ready().await?;

    file.flush()
        .map_err(|e| format!("Partial model flush error: {}", e))?;
    file.sync_all()
        .map_err(|e| format!("Partial model sync error: {}", e))?;
    drop(file);

    if let Some(total) = total {
        if downloaded != total {
            return Err(format!(
                "Download ended at {} bytes, expected {} bytes",
                downloaded, total
            ));
        }
    }

    fs::rename(&part_path, destination)
        .map_err(|e| format!("Atomic model install error: {}", e))?;
    emit_progress(&mut progress, downloaded, total, "complete");

    Ok(downloaded)
}

fn emit_progress<F>(progress: &mut F, downloaded: u64, total: Option<u64>, status: &str)
where
    F: FnMut(ModelDownloadProgress),
{
    progress(ModelDownloadProgress {
        downloaded,
        total: total.unwrap_or(0),
        status: status.to_string(),
        resumable: true,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use futures_util::stream;
    use std::fs;

    #[tokio::test]
    async fn stream_failure_keeps_partial_file_and_does_not_create_final_model() {
        let tmp = tempfile::tempdir().unwrap();
        let final_path = tmp.path().join("clip-vit-b32-vision.onnx");
        let stream = stream::iter(vec![
            Ok(Bytes::from_static(b"partial")),
            Err("network reset"),
        ]);

        let result = write_stream_to_model_file(&final_path, 0, Some(14), stream, |_| {}).await;

        assert!(result.is_err());
        assert!(!final_path.exists());
        assert_eq!(fs::read(part_path_for(&final_path)).unwrap(), b"partial");
    }

    #[tokio::test]
    async fn completed_stream_atomically_promotes_part_file_to_final_model() {
        let tmp = tempfile::tempdir().unwrap();
        let final_path = tmp.path().join("clip-vit-b32-vision.onnx");
        let stream = stream::iter(vec![
            Ok::<Bytes, &'static str>(Bytes::from_static(b"model ")),
            Ok::<Bytes, &'static str>(Bytes::from_static(b"bytes")),
        ]);

        write_stream_to_model_file(&final_path, 0, Some(11), stream, |_| {})
            .await
            .unwrap();

        assert_eq!(fs::read(&final_path).unwrap(), b"model bytes");
        assert!(!part_path_for(&final_path).exists());
    }

    #[tokio::test]
    async fn resumed_stream_appends_to_existing_part_file_before_promotion() {
        let tmp = tempfile::tempdir().unwrap();
        let final_path = tmp.path().join("clip-vit-b32-vision.onnx");
        let part_path = part_path_for(&final_path);
        fs::write(&part_path, b"model ").unwrap();
        let stream = stream::iter(vec![Ok::<Bytes, &'static str>(Bytes::from_static(
            b"bytes",
        ))]);

        write_stream_to_model_file(&final_path, 6, Some(11), stream, |_| {})
            .await
            .unwrap();

        assert_eq!(fs::read(&final_path).unwrap(), b"model bytes");
        assert!(!part_path.exists());
    }

    #[tokio::test]
    async fn cancelled_stream_keeps_partial_file_and_does_not_create_final_model() {
        let tmp = tempfile::tempdir().unwrap();
        let final_path = tmp.path().join("clip-vit-b32-vision.onnx");
        let part_path = part_path_for(&final_path);
        let cancel = tokio_util::sync::CancellationToken::new();
        let control = DownloadControl::new(cancel.clone(), PauseController::default());
        let stream = stream::iter(vec![
            Ok::<Bytes, &'static str>(Bytes::from_static(b"partial")),
            Ok::<Bytes, &'static str>(Bytes::from_static(b" bytes")),
        ]);

        let result = write_stream_to_model_file_controlled(
            &final_path,
            0,
            Some(13),
            stream,
            &control,
            |p| {
                if p.downloaded >= 7 {
                    cancel.cancel();
                }
            },
        )
        .await;

        assert!(result.unwrap_err().contains("cancelled"));
        assert!(!final_path.exists());
        assert_eq!(fs::read(part_path).unwrap(), b"partial");
    }

    #[tokio::test]
    async fn paused_stream_waits_until_resumed_before_finishing() {
        let tmp = tempfile::tempdir().unwrap();
        let final_path = tmp.path().join("clip-vit-b32-vision.onnx");
        let pause = PauseController::default();
        pause.pause();
        let control =
            DownloadControl::new(tokio_util::sync::CancellationToken::new(), pause.clone());
        let stream = stream::iter(vec![Ok::<Bytes, &'static str>(Bytes::from_static(
            b"model bytes",
        ))]);
        let task_path = final_path.clone();

        let task = tokio::spawn(async move {
            write_stream_to_model_file_controlled(&task_path, 0, Some(11), stream, &control, |_| {})
                .await
        });

        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        assert!(!final_path.exists());

        pause.resume();
        task.await.unwrap().unwrap();
        assert_eq!(fs::read(&final_path).unwrap(), b"model bytes");
    }
}
