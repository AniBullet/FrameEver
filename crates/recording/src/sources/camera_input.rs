use crate::{
    feeds::camera::{self, CameraFeedLock},
    output_pipeline::NativeCameraFrame,
};
use anyhow::{anyhow, bail};
use std::{
    sync::{Arc, Mutex, atomic::AtomicU64},
    time::Duration,
};

const CAMERA_SENDER_TIMEOUT: Duration = Duration::from_millis(1500);

#[derive(Clone)]
pub struct CameraTrackInput {
    state: Arc<Mutex<InputState>>,
}

struct InputState {
    lock: Option<Arc<CameraFeedLock>>,
    track: Option<TrackQueue>,
    switchable: bool,
}

struct TrackQueue {
    sender: flume::WeakSender<NativeCameraFrame>,
    dropped: Arc<AtomicU64>,
    bridge: Option<flume::Sender<NativeCameraFrame>>,
    next: flume::Sender<flume::Receiver<NativeCameraFrame>>,
}

pub(crate) struct TrackReceiver<T> {
    rx: flume::Receiver<T>,
    next: flume::Receiver<flume::Receiver<T>>,
}

impl<T> TrackReceiver<T> {
    #[cfg(test)]
    pub(crate) fn fixed(rx: flume::Receiver<T>) -> Self {
        Self {
            rx,
            next: flume::bounded(1).1,
        }
    }

    pub(crate) async fn recv(&mut self) -> Result<T, flume::RecvError> {
        loop {
            tokio::select! {
                biased;
                Ok(rx) = self.next.recv_async() => self.rx = rx,
                frame = self.rx.recv_async() => match frame {
                    Ok(frame) => return Ok(frame),
                    Err(closed) => match self.next.try_recv() {
                        Ok(rx) => self.rx = rx,
                        Err(_) => return Err(closed),
                    },
                },
            }
        }
    }
}

async fn add_sender(
    lock: &CameraFeedLock,
    sender: flume::Sender<NativeCameraFrame>,
    dropped: Arc<AtomicU64>,
) -> anyhow::Result<()> {
    tokio::time::timeout(
        CAMERA_SENDER_TIMEOUT,
        lock.ask(camera::AddCountedNativeSender(sender, dropped)),
    )
    .await
    .map_err(|_| anyhow!("Timed out adding the camera sender"))?
    .map_err(|error| anyhow!("Failed to add the camera sender: {error}"))
}

impl CameraTrackInput {
    pub fn new(lock: Arc<CameraFeedLock>) -> Self {
        Self {
            state: Arc::new(Mutex::new(InputState {
                lock: Some(lock),
                track: None,
                switchable: cfg!(not(windows)),
            })),
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, InputState> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }

    pub fn lock(&self) -> Option<Arc<CameraFeedLock>> {
        self.state().lock.clone()
    }

    pub(crate) async fn connect(
        &self,
        dropped: Arc<AtomicU64>,
    ) -> anyhow::Result<TrackReceiver<NativeCameraFrame>> {
        let lock = self
            .lock()
            .ok_or_else(|| anyhow!("The camera track has no camera"))?;
        let (sender, rx) = flume::bounded(super::native_ingress_capacity(lock.video_info()));
        add_sender(&lock, sender.clone(), dropped.clone()).await?;
        let (next, next_rx) = flume::bounded(1);
        self.state().track = Some(TrackQueue {
            sender: sender.downgrade(),
            dropped,
            bridge: None,
            next,
        });
        Ok(TrackReceiver { rx, next: next_rx })
    }

    pub(crate) fn set_switchable(&self, switchable: bool) {
        self.state().switchable = switchable;
    }

    pub fn is_released(&self) -> bool {
        self.state()
            .track
            .as_ref()
            .is_some_and(|track| track.bridge.is_some())
    }

    pub async fn release(&self) -> anyhow::Result<()> {
        let (lock, bridge) = {
            let mut state = self.state();
            let switchable = state.switchable;
            let Some(track) = state.track.as_mut() else {
                bail!("The camera track is not recording");
            };
            if track.bridge.is_some() {
                bail!("The camera track is already released");
            }
            if !switchable {
                bail!("The camera track cannot take another camera's frames");
            }
            let Some(bridge) = track.sender.upgrade() else {
                bail!("The camera track has ended");
            };
            track.bridge = Some(bridge.clone());
            (state.lock.take(), bridge)
        };
        if let Some(lock) = lock {
            match tokio::time::timeout(
                CAMERA_SENDER_TIMEOUT,
                lock.ask(camera::RemoveNativeSender(bridge)),
            )
            .await
            {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    tracing::warn!("Removing the camera track from its feed failed: {error}")
                }
                Err(_) => tracing::warn!("Timed out removing the camera track from its feed"),
            }
        }
        Ok(())
    }

    pub async fn attach(&self, lock: Arc<CameraFeedLock>) -> anyhow::Result<()> {
        let (dropped, next) = {
            let state = self.state();
            let Some(track) = state.track.as_ref().filter(|track| track.bridge.is_some()) else {
                bail!("The camera track is not released");
            };
            (track.dropped.clone(), track.next.clone())
        };
        let (sender, rx) = flume::bounded(super::native_ingress_capacity(lock.video_info()));
        add_sender(&lock, sender.clone(), dropped).await?;
        if !matches!(
            tokio::time::timeout(CAMERA_SENDER_TIMEOUT, next.send_async(rx)).await,
            Ok(Ok(()))
        ) {
            tokio::time::timeout(
                CAMERA_SENDER_TIMEOUT,
                lock.ask(camera::RemoveNativeSender(sender)),
            )
            .await
            .ok();
            bail!("The camera track stopped taking frames");
        }
        let mut state = self.state();
        state.lock = Some(lock);
        if let Some(track) = state.track.as_mut() {
            track.sender = sender.downgrade();
            track.bridge = None;
        }
        Ok(())
    }
}

#[cfg(all(test, windows))]
pub(crate) mod tests {
    use super::*;
    use crate::feeds::camera::synthetic::SyntheticCamera;
    use frameever_camera_windows::PixelFormat;
    use frameever_media_info::{RawVideoFormat, VideoInfo};
    use frameever_mediafoundation_utils::IMFMediaBufferExt;
    use frameever_timestamp::Timestamp;
    use windows::Win32::Media::MediaFoundation::MFCreateMemoryBuffer;

    pub(crate) fn frame_from(
        pixel_format: PixelFormat,
        width: u32,
        height: u32,
        bytes: &[u8],
    ) -> NativeCameraFrame {
        let buffer = unsafe { MFCreateMemoryBuffer(bytes.len() as u32) }.unwrap();
        buffer.lock_for_write().unwrap().copy_from_slice(bytes);
        unsafe { buffer.SetCurrentLength(bytes.len() as u32) }.unwrap();
        #[allow(clippy::arc_with_non_send_sync)]
        let buffer = Arc::new(Mutex::new(buffer));
        NativeCameraFrame {
            buffer,
            pixel_format,
            width,
            height,
            is_bottom_up: false,
            timestamp: Timestamp::Instant(std::time::Instant::now()),
        }
    }

    fn nv12(width: u32, height: u32) -> NativeCameraFrame {
        let len = (width * height * 3 / 2) as usize;
        frame_from(PixelFormat::NV12, width, height, &vec![16; len])
    }

    async fn next(rx: &mut TrackReceiver<NativeCameraFrame>) -> Option<(u32, u32)> {
        tokio::time::timeout(Duration::from_millis(500), rx.recv())
            .await
            .ok()?
            .ok()
            .map(|frame| (frame.width, frame.height))
    }

    #[tokio::test]
    async fn a_released_track_moves_to_another_camera_without_closing() {
        let first = SyntheticCamera::spawn(
            "first",
            VideoInfo::from_raw(RawVideoFormat::Nv12, 64, 36, 30),
        );
        let second = SyntheticCamera::spawn(
            "second",
            VideoInfo::from_raw(RawVideoFormat::Nv12, 96, 54, 60),
        );
        let input = CameraTrackInput::new(Arc::new(first.lock().await.unwrap()));
        let mut rx = input.connect(Default::default()).await.unwrap();
        input.set_switchable(true);

        first.push_native(nv12(64, 36)).await;
        assert_eq!(next(&mut rx).await, Some((64, 36)));

        input.release().await.unwrap();
        assert!(input.is_released());
        assert!(input.lock().is_none());
        first.push_native(nv12(64, 36)).await;
        assert_eq!(next(&mut rx).await, None);
        drop(first.lock().await.expect("the released camera is unlocked"));

        input
            .attach(Arc::new(second.lock().await.unwrap()))
            .await
            .unwrap();
        assert!(!input.is_released());
        second.push_native(nv12(96, 54)).await;
        assert_eq!(next(&mut rx).await, Some((96, 54)));
        first.push_native(nv12(64, 36)).await;
        assert_eq!(next(&mut rx).await, None);

        drop(input);
        second.remove_input().await;
        assert!(
            tokio::time::timeout(Duration::from_millis(500), rx.recv())
                .await
                .is_ok_and(|result| result.is_err()),
            "the track sees its feed close once no switch holds it open"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_camera_track_keeps_recording_through_a_switch_to_another_size_and_format() {
        use crate::output_pipeline::{OutputPipeline, WindowsCameraMuxer};
        use crate::sources::NativeCamera;

        ffmpeg::init().unwrap();
        let first = SyntheticCamera::spawn(
            "first",
            VideoInfo::from_raw(RawVideoFormat::Nv12, 320, 240, 30),
        );
        let second = SyntheticCamera::spawn(
            "second",
            VideoInfo::from_raw(RawVideoFormat::Bgra, 640, 360, 30),
        );
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("camera.mp4");
        let input = CameraTrackInput::new(Arc::new(first.lock().await.unwrap()));
        let pipeline = OutputPipeline::builder(path.clone())
            .with_video::<NativeCamera>(input.clone())
            .with_timestamps(frameever_timestamp::Timestamps::now())
            .build::<WindowsCameraMuxer>(Default::default())
            .await
            .unwrap();

        let frame_interval = Duration::from_millis(33);
        for _ in 0..20 {
            first.push_native(nv12(320, 240)).await;
            tokio::time::sleep(frame_interval).await;
        }
        input.release().await.unwrap();
        input
            .attach(Arc::new(second.lock().await.unwrap()))
            .await
            .unwrap();
        let white = vec![255; 640 * 360 * 4];
        for _ in 0..20 {
            second
                .push_native(frame_from(PixelFormat::RGB32, 640, 360, &white))
                .await;
            tokio::time::sleep(frame_interval).await;
        }
        pipeline.stop().await.unwrap();

        let mut file = ffmpeg::format::input(&path).unwrap();
        let stream = file.streams().best(ffmpeg::media::Type::Video).unwrap();
        let index = stream.index();
        let mut decoder = ffmpeg::codec::Context::from_parameters(stream.parameters())
            .unwrap()
            .decoder()
            .video()
            .unwrap();
        assert_eq!((decoder.width(), decoder.height()), (320, 240));
        let mut frames = Vec::new();
        for (stream, packet) in file.packets() {
            if stream.index() != index {
                continue;
            }
            decoder.send_packet(&packet).unwrap();
            let mut frame = ffmpeg::frame::Video::empty();
            while decoder.receive_frame(&mut frame).is_ok() {
                frames.push(frame.data(0)[frame.stride(0) * 120 + 160]);
            }
        }
        decoder.send_eof().unwrap();
        let mut frame = ffmpeg::frame::Video::empty();
        while decoder.receive_frame(&mut frame).is_ok() {
            frames.push(frame.data(0)[frame.stride(0) * 120 + 160]);
        }
        assert!(frames.len() >= 30, "{} frames", frames.len());
        assert!(frames[..5].iter().all(|&luma| luma < 60), "{frames:?}");
        assert!(
            frames[frames.len() - 5..].iter().all(|&luma| luma > 200),
            "the second camera's white frames reach the track: {frames:?}"
        );
    }

    #[tokio::test]
    async fn only_a_released_track_takes_another_camera() {
        let first = SyntheticCamera::spawn(
            "first",
            VideoInfo::from_raw(RawVideoFormat::Nv12, 64, 36, 30),
        );
        let second = SyntheticCamera::spawn(
            "second",
            VideoInfo::from_raw(RawVideoFormat::Nv12, 64, 36, 30),
        );
        let input = CameraTrackInput::new(Arc::new(first.lock().await.unwrap()));
        assert!(input.release().await.is_err());
        let _rx = input.connect(Default::default()).await.unwrap();
        assert!(
            input.release().await.is_err(),
            "a track whose format is not known to be writable keeps its camera"
        );
        assert!(!input.is_released());
        input.set_switchable(true);
        assert!(
            input
                .attach(Arc::new(second.lock().await.unwrap()))
                .await
                .is_err()
        );
        input.release().await.unwrap();
        assert!(input.release().await.is_err());
    }
}
