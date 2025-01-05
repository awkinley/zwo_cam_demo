use std::thread::sleep;

use asi::{ASICloseCamera, ROIFormat, ASI_ERROR, CONTROL_TYPE, EXPOSURE_STATUS};

pub mod asi;
pub mod camera_controller;

impl asi::_ASI_CAMERA_INFO {
    pub fn new() -> Self {
        Self {
            Name: [0; 64],
            CameraID: 0,
            MaxHeight: 0,
            MaxWidth: 0,
            IsColorCam: 1,
            BayerPattern: 1,
            SupportedBins: [0; 16],
            SupportedVideoFormat: [0; 8],
            PixelSize: 0.0,
            MechanicalShutter: 0,
            ST4Port: 0,
            IsCoolerCam: 0,
            IsUSB3Host: 0,
            IsUSB3Camera: 0,
            ElecPerADU: 0.0,
            BitDepth: 0,
            IsTriggerCam: 0,
            Unused: [0; 16],
        }
    }
}

impl Default for asi::_ASI_CAMERA_INFO {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
pub struct Camera {
    info: asi::_ASI_CAMERA_INFO,
}

impl Camera {
    pub fn new(info: asi::_ASI_CAMERA_INFO) -> Self {
        Self { info }
    }

    pub fn open(&self) -> Result<OpenCamera<'_>, ASI_ERROR> {
        OpenCamera::new(self)
    }

    pub fn get_name(&self) -> String {
        String::from_utf8(self.info.Name.iter().map(|v| *v as u8).collect()).unwrap()
    }
}

pub struct OpenCamera<'a> {
    camera: &'a Camera,
}

impl OpenCamera<'static> {
    pub fn make_static<'a>(ccd: &OpenCamera<'a>) -> Self {
        unsafe {
            Self {
                camera: std::mem::transmute::<&'a Camera, &'static Camera>(ccd.camera),
            }
        }
    }
}

impl<'a> OpenCamera<'a> {
    pub fn new(camera: &'a Camera) -> Result<Self, ASI_ERROR> {
        unsafe { asi::open_camera(camera.info.CameraID)? }
        Ok(Self { camera })
    }

    pub fn id(&self) -> i32 {
        self.camera.info.CameraID
    }
    pub fn init(&self) -> Result<(), ASI_ERROR> {
        unsafe { asi::init_camera(self.id()) }
    }

    pub fn set_control_value(
        &self,
        control_type: CONTROL_TYPE,
        value: i64,
        auto: bool,
    ) -> Result<(), ASI_ERROR> {
        unsafe { asi::set_control_value(self.id(), control_type, value, auto) }
    }

    pub fn get_control_value(&self, control_type: CONTROL_TYPE) -> Result<(i64, bool), ASI_ERROR> {
        unsafe {
            let mut value: i64 = 0;
            let mut auto: i32 = 0;
            asi::get_control_value(self.id(), control_type, &mut value, &mut auto)?;
            Ok((value, auto > 0))
        }
    }

    pub fn set_roi_format(
        &self,
        width: i32,
        height: i32,
        bin: i32,
        img_type: asi::IMG_TYPE,
    ) -> Result<(), ASI_ERROR> {
        unsafe { asi::set_roi_format(self.id(), width, height, bin, img_type) }
    }

    pub fn get_roi_format(&self) -> Result<ROIFormat, ASI_ERROR> {
        unsafe { asi::get_roi_format(self.id()) }
    }

    pub fn start_exposure(&self) -> Result<(), ASI_ERROR> {
        unsafe { asi::start_exposure(self.id(), false) }
    }

    pub fn get_exp_status(&self) -> Result<EXPOSURE_STATUS, ASI_ERROR> {
        unsafe { asi::get_exp_status(self.id()) }
    }

    pub fn get_data_after_exp(&self, data: &mut [u8]) -> Result<(), ASI_ERROR> {
        unsafe { asi::get_data_after_exp(self.id(), data.as_mut_ptr(), data.len() as i64) }
    }

    pub fn take_exposure(&self, data: &mut [u8]) -> Result<(), ASI_ERROR> {
        self.start_exposure()?;
        let mut loop_count = 0;
        while self.get_exp_status()? == asi::EXPOSURE_STATUS::EXP_WORKING {
            sleep(std::time::Duration::from_millis(1));
            loop_count += 1;
        }

        std::dbg!(loop_count);
        if self.get_exp_status()? == asi::EXPOSURE_STATUS::EXP_SUCCESS {
            self.get_data_after_exp(data)?;
            Ok(())
        } else {
            Err(ASI_ERROR::UNKNOWN)
        }
    }

    pub fn start_video_capture(&self) -> Result<(), ASI_ERROR> {
        unsafe { asi::start_video_capture(self.id()) }
    }
    pub fn stop_video_capture(&self) -> Result<(), ASI_ERROR> {
        unsafe { asi::stop_video_capture(self.id()) }
    }

    pub fn get_dropped_frames(&self) -> Result<i32, ASI_ERROR> {
        unsafe { asi::get_dropped_frames(self.id()) }
    }

    pub fn get_video_data(&self, data: &mut [u8], wait_ms: i32) -> Result<(), ASI_ERROR> {
        unsafe { asi::get_video_data(self.id(), data.as_mut_ptr(), data.len() as i64, wait_ms) }
    }
}

impl Drop for OpenCamera<'_> {
    fn drop(&mut self) {
        println!("Closing Camera {}", self.camera.get_name());
        unsafe {
            ASICloseCamera(self.camera.info.CameraID);
        }
    }
}
