use std::{
    ops::Deref,
    sync::{atomic::AtomicBool, Arc, Mutex},
    thread::{sleep, JoinHandle},
    time::Duration,
};

use opencv::{
    core::{Mat, MatTraitManual, Scalar, CV_8UC3},
    imgcodecs,
};
use serde::Serialize;
use serde_repr::Serialize_repr;

use anyhow::{anyhow, Result};
use image::{ImageBuffer, Rgb, RgbImage};

use imageproc::stats::ChannelHistogram;

use plotters::prelude::*;
use tokio::sync::broadcast::{self, Receiver, Sender};

use crate::{
    asi::{self, ASI_ERROR},
    Camera, OpenCamera,
};

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ControlMessages {
    StartPreview,
    StopPreview,
    SetGain(i32),
    SetExposure(f32),
    SetWbR(i32),
    SetWbB(i32),
    SwitchOutput,
    StartCapture(i32),
}

pub fn histogram<P, Container>(image: &ImageBuffer<P, Container>) -> ChannelHistogram
where
    P: image::Pixel<Subpixel = u8>,
    Container: Deref<Target = [P::Subpixel]>,
{
    let mut hist = vec![[0u32; 256]; P::CHANNEL_COUNT as usize];

    for pix in image.pixels() {
        for (i, c) in pix.channels().iter().enumerate() {
            hist[i][*c as usize] += 1;
        }
    }

    ChannelHistogram { channels: hist }
}

fn make_hist_plot(hist: &ChannelHistogram) -> RgbImage {
    let mut img = RgbImage::new(1920, 1080);
    {
        let drawing_area =
            BitMapBackend::with_buffer(img.as_flat_samples_mut().samples, (1920, 1080))
                .into_drawing_area();

        drawing_area.fill(&WHITE).unwrap();
        let max_value = hist
            .channels
            .iter()
            .filter_map(|v| (v.iter().max().copied()))
            .max()
            .unwrap_or(1);

        let mut ctx = ChartBuilder::on(&drawing_area)
            .set_label_area_size(LabelAreaPosition::Left, 40)
            .set_label_area_size(LabelAreaPosition::Bottom, 40)
            .build_cartesian_2d(0..255, 0..max_value)
            .unwrap();
        ctx.configure_mesh().draw().unwrap();

        for (channel, color) in hist.channels.iter().zip([&RED, &GREEN, &BLUE]) {
            ctx.draw_series(LineSeries::new(
                channel
                    .iter()
                    .enumerate()
                    .map(|(idx, v)| (idx as i32, *v))
                    .collect::<Vec<_>>(),
                color,
            ))
            .unwrap();
        }
    }

    img
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type")]
pub enum ClientPacket {
    Preview(ImagePacket),
    CaptureStatus(CaptureStatus),
}

#[derive(Clone, Debug, Serialize)]
pub struct CaptureStatus {
    captured_frames: i32,
    total_frames: i32,
}

#[derive(Clone, Debug, Serialize_repr)]
#[repr(u8)]
pub enum PixelOrder {
    BGR = 0,
    RGB = 1,
}

#[derive(Clone, Debug, Serialize)]
pub struct ImagePacket {
    pub w: u32,
    pub h: u32,
    pub pix: PixelOrder,
    #[serde(with = "serde_bytes")]
    pub img: Vec<u8>,
    pub controls: ControlValues,
}

#[derive(Clone, Debug, Serialize)]
pub struct ControlValues {
    pub gain: i64,
    pub exposure: f64,
    pub wb_r: i64,
    pub wb_b: i64,
}

enum CamState {
    Stopped,
    Preview { show_hist: bool },
    Capture { total_frames: i32 },
}

struct VideoStreamer<'a> {
    ccd: OpenCamera<'a>,
    latest_frame: Arc<Mutex<Vec<u8>>>,
    current_frame: Vec<u8>,
    stop_msg: Receiver<bool>,
    frame_available: Arc<AtomicBool>,
}

impl<'a> VideoStreamer<'a> {
    pub fn new(
        ccd: OpenCamera<'a>,
        stop_msg: Receiver<bool>,
        latest_frame: Arc<Mutex<Vec<u8>>>,
        frame_available: Arc<AtomicBool>,
    ) -> Result<VideoStreamer<'a>> {
        let roi_format = ccd.get_roi_format()?;
        let buf_size = roi_format.width * roi_format.height * roi_format.img_type.bytes_per_pixel();
        let buf_one = vec![0; buf_size as usize];
        let buf_two = vec![0; buf_size as usize];
        {
            let mut locked_latest = latest_frame.lock().unwrap();
            *locked_latest = buf_one;
        }

        Ok(Self {
            ccd,
            latest_frame,
            current_frame: buf_two,
            stop_msg,
            frame_available,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        self.ccd.start_video_capture()?;
        loop {
            self.ccd.get_video_data(&mut self.current_frame, 500)?;

            let mut latest_frame = self.latest_frame.lock().unwrap();
            std::mem::swap(&mut self.current_frame, &mut latest_frame);
            self.frame_available
                .store(true, std::sync::atomic::Ordering::Relaxed);

            if !self.stop_msg.is_empty() {
                break;
            }
        }
        self.ccd.stop_video_capture()?;
        Ok(())
    }
}

pub struct CameraController<'a> {
    camera: &'a Camera,
    ccd: OpenCamera<'a>,
    tx: Sender<ClientPacket>,
    rx: Receiver<ControlMessages>,
    state: CamState,
    width: u32,
    height: u32,
    stop_msg: Option<Sender<bool>>,
    latest_frame: Arc<Mutex<Vec<u8>>>,
    frame_available: Arc<AtomicBool>,
    streamer_thread: Option<JoinHandle<()>>,
}

impl<'a> CameraController<'a> {
    pub fn new(
        camera: &'a Camera,
        tx: Sender<ClientPacket>,
        rx: Receiver<ControlMessages>,
    ) -> Result<Self> {
        println!("Camera: {}", camera.get_name());
        let ccd = camera.open()?;
        ccd.init()?;
        Ok(Self {
            camera,
            ccd,
            tx,
            rx,
            state: CamState::Stopped,
            width: 1920,
            height: 1080,
            stop_msg: None,
            latest_frame: Arc::new(Mutex::new(Vec::new())),
            frame_available: Arc::new(AtomicBool::new(false)),
            streamer_thread: None,
        })
    }

    fn set_gain(&self, gain: i32, auto: bool) -> Result<(), ASI_ERROR> {
        self.ccd
            .set_control_value(asi::CONTROL_TYPE::GAIN, gain as i64, auto)
    }

    fn set_exposure(&self, exp: f32, auto: bool) -> Result<(), ASI_ERROR> {
        self.ccd.set_control_value(
            asi::CONTROL_TYPE::EXPOSURE,
            (exp * 1000.).trunc() as i64,
            auto,
        )
    }

    fn set_white_balance_red(&self, r: i32, auto: bool) -> Result<(), ASI_ERROR> {
        self.ccd
            .set_control_value(asi::CONTROL_TYPE::WB_R, r as i64, auto)
    }

    fn set_white_balance_blue(&self, b: i32, auto: bool) -> Result<(), ASI_ERROR> {
        self.ccd
            .set_control_value(asi::CONTROL_TYPE::WB_B, b as i64, auto)
    }

    fn start_video(&mut self) -> Result<()> {
        match self.state {
            CamState::Stopped => {
                let (tx, rx) = broadcast::channel(1);

                let static_camera = OpenCamera::make_static(&self.ccd);
                let thread_frame_avail = self.frame_available.clone();
                let thread_latest_frame = self.latest_frame.clone();
                let thread = std::thread::spawn(move || {
                    let mut streamer = VideoStreamer::new(
                        static_camera,
                        rx,
                        thread_latest_frame,
                        thread_frame_avail,
                    )
                    .unwrap();
                    streamer.run().unwrap();
                });
                self.stop_msg = Some(tx);
                self.streamer_thread = Some(thread);
                // self.ccd.start_video_capture()?;
                self.state = CamState::Preview { show_hist: false };
                println!("Starting camera video");
                Ok(())
            }
            CamState::Preview { show_hist: _ } => Ok(()),
            CamState::Capture { total_frames: _ } => {
                panic!("Tried to start the video while capturing?")
            }
        }
    }

    fn stop_video(&mut self) -> Result<()> {
        match self.state {
            CamState::Stopped => Ok(()),
            CamState::Preview { show_hist: _ } => {
                if let Some(tx) = self.stop_msg.take() {
                    tx.send(true)?;
                }
                if let Some(handle) = self.streamer_thread.take() {
                    handle.join().unwrap();
                }
                // self.ccd.stop_video_capture()?;
                self.state = CamState::Stopped;
                println!("Stopped camera video");
                Ok(())
            }
            CamState::Capture { total_frames: _ } => {
                panic!("Tried to start the video while capturing?")
            }
        }
    }

    fn handle_command(&mut self, cmd: ControlMessages) -> Result<()> {
        println!("Received command {:?}", cmd);
        match cmd {
            ControlMessages::SetGain(gain) => self.set_gain(gain, false)?,
            ControlMessages::SetExposure(exp) => self.set_exposure(exp, false)?,
            ControlMessages::SetWbR(r) => self.set_white_balance_red(r, false)?,
            ControlMessages::SetWbB(b) => self.set_white_balance_blue(b, false)?,
            ControlMessages::SwitchOutput => {
                if let CamState::Preview { show_hist } = &mut self.state {
                    *show_hist = !*show_hist;
                }
            }
            ControlMessages::StartPreview => self.start_video()?,
            ControlMessages::StopPreview => self.stop_video()?,
            ControlMessages::StartCapture(total_frames) => {
                self.state = CamState::Capture { total_frames }
            }
        }
        Ok(())
    }

    fn handle_commands(&mut self) -> Result<()> {
        loop {
            if self.rx.is_empty() {
                return Ok(());
            }
            if let Ok(cmd) = self.rx.blocking_recv() {
                self.handle_command(cmd)?;
            }
        }
    }

    fn get_controls(&self) -> Result<ControlValues> {
        Ok(ControlValues {
            gain: self.ccd.get_control_value(asi::CONTROL_TYPE::GAIN)?.0,
            exposure: self.ccd.get_control_value(asi::CONTROL_TYPE::EXPOSURE)?.0 as f64 / 1000.,
            wb_b: self.ccd.get_control_value(asi::CONTROL_TYPE::WB_B)?.0,
            wb_r: self.ccd.get_control_value(asi::CONTROL_TYPE::WB_R)?.0,
        })
    }
    fn make_preview(&self, img_buffer: &mut [u8]) -> Result<ClientPacket> {
        // let start = std::time::Instant::now();
        //self.ccd.take_exposure(img_buffer);
        // self.ccd.get_video_data(img_buffer, 500)?;
        while !self
            .frame_available
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            sleep(Duration::from_micros(500));
        }
        let img_buffer = self.latest_frame.lock().unwrap();
        self.frame_available
            .store(false, std::sync::atomic::Ordering::Release);
        // let end = std::time::Instant::now();
        // let dropped_frames = self.ccd.get_dropped_frames()?;
        // println!(
        //     "Get video data took {:?}, dropped {dropped_frames} frames",
        //     end - start
        // );
        // println!("Get data for preview in {:?}", end - start);

        Ok(ClientPacket::Preview(ImagePacket {
            w: self.width,
            h: self.height,
            pix: PixelOrder::BGR,
            img: img_buffer.to_vec(),
            controls: self.get_controls()?,
        }))
    }

    fn make_histogram(&self, img: &mut ImageBuffer<Rgb<u8>, Vec<u8>>) -> Result<ClientPacket> {
        let start = std::time::Instant::now();
        while !self
            .frame_available
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            sleep(Duration::from_micros(500));
        }
        let img_buffer = self.latest_frame.lock().unwrap();
        self.frame_available
            .store(false, std::sync::atomic::Ordering::Release);
        //self.ccd.take_exposure(img.as_flat_samples_mut().samples);
        // self.ccd
        //     .get_video_data(img.as_flat_samples_mut().samples, 500)?;
        let img = ImageBuffer::<Rgb<u8>, _>::from_raw(self.width, self.height, &img_buffer[..])
            .ok_or(anyhow!("ImageBuffer::from_raw failed!"))?;

        let end = std::time::Instant::now();
        // let dropped_frames = self.ccd.get_dropped_frames()?;
        // println!("Get image data for histogram took {:?}", end - start);
        // let mut img = ImageBuffer::<Rgb<u8>, &[u8]>::new(self.width, self.height);

        let mut hist_result = histogram(&img);
        // handle bgr -> rgb conversion
        hist_result.channels.swap(0, 2);

        let hist_img = make_hist_plot(&hist_result);
        // hist_img.write_to(&mut Cursor::new(&mut png_bytes), image::ImageFormat::Jpeg)?;
        Ok(ClientPacket::Preview(ImagePacket {
            w: self.width,
            h: self.height,
            pix: PixelOrder::RGB,
            img: hist_img.into_vec(),
            controls: self.get_controls()?,
        }))
    }

    fn capture_loop(&self, total_frames: i32) -> Result<()> {
        println!("Starting capture loop");

        let typ = CV_8UC3;
        let mut frame = Mat::new_rows_cols_with_default(
            self.width as i32,
            self.height as i32,
            typ,
            Scalar::all(0.),
        )
        .unwrap();

        let mut params = opencv::core::Vector::<i32>::new();
        params.push(opencv::imgcodecs::IMWRITE_TIFF_COMPRESSION);
        params.push(1);

        for i in 0..total_frames {
            self.ccd.get_video_data(frame.data_bytes_mut()?, 500)?;

            let file_name = format!("frame_{i}.tiff");
            imgcodecs::imwrite(&file_name, &frame, &params)?;
            self.tx.send(ClientPacket::CaptureStatus(CaptureStatus {
                captured_frames: i + 1,
                total_frames,
            }))?;
        }
        println!("Finished capture loop");

        Ok(())
    }

    pub fn run(&mut self) -> Result<()> {
        let bin = 1;
        self.width = 1920 / bin;
        self.height = 1080 / bin;

        self.ccd.set_roi_format(
            self.width as i32,
            self.height as i32,
            bin as i32,
            asi::IMG_TYPE::RGB24,
        )?;

        self.set_gain(200, false)?;
        self.set_exposure(10.0, false)?;
        self.set_white_balance_blue(87, false)?;
        self.set_white_balance_red(45, false)?;
        self.ccd
            .set_control_value(asi::CONTROL_TYPE::BANDWIDTHOVERLOAD, 100, false)?;

        let buf_size = self.width * self.height * 3;
        let mut img = RgbImage::new(self.width as u32, self.height as u32);
        let mut img_buffer = vec![0; buf_size as usize];

        loop {
            use CamState::*;
            match self.state {
                Stopped => sleep(std::time::Duration::from_millis(1)),
                Preview { show_hist } => {
                    if self.tx.len() >= 1 {
                        sleep(std::time::Duration::from_millis(1));
                    } else {
                        if show_hist {
                            self.tx.send(self.make_histogram(&mut img)?)?;
                        } else {
                            let start = std::time::Instant::now();
                            self.tx.send(self.make_preview(&mut img_buffer)?)?;
                            let stop = std::time::Instant::now();
                            println!("Make preview took {:?}", stop - start);
                        }
                    }
                }
                Capture { total_frames } => {
                    self.capture_loop(total_frames)?;
                    self.state = Preview { show_hist: false };
                }
            }

            self.handle_commands()?;
        }
    }
}
