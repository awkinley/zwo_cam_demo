#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use thiserror::Error;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[derive(Error, Debug, PartialEq, Eq)]
pub enum ASI_ERROR {
    //ASI ERROR CODE
    #[error("no camera connected or index value out of boundary")]
    INVALID_INDEX,
    #[error("invalid ID")]
    INVALID_ID,
    #[error("invalid control type")]
    INVALID_CONTROL_TYPE,
    #[error("camera didn't open")]
    CAMERA_CLOSED,
    #[error("failed to find the camera, maybe the camera has been removed")]
    CAMERA_REMOVED,
    #[error("cannot find the path of the file")]
    INVALID_PATH,
    #[error("invalid file format")]
    INVALID_FILEFORMAT,
    #[error("wrong video format size")]
    INVALID_SIZE,
    #[error("unsupported image formate")]
    INVALID_IMGTYPE,
    #[error("the startpos is out of boundary")]
    OUTOF_BOUNDARY,
    #[error("timeout")]
    TIMEOUT,
    #[error("stop capture first")]
    INVALID_SEQUENCE,
    #[error("buffer size is not big enough")]
    BUFFER_TOO_SMALL,
    #[error("Video mode active")]
    VIDEO_MODE_ACTIVE,
    #[error("Exposuer in progress")]
    EXPOSURE_IN_PROGRESS,
    #[error("general error, eg: value is out of valid range")]
    GENERAL_ERROR,
    #[error("the current mode is wrong")]
    INVALID_MODE,
    #[error("this camera do not support GPS")]
    GPS_NOT_SUPPORTED,
    #[error("the FPGA GPS ver is too low")]
    GPS_VER_ERR,
    #[error("failed to read or write data to FPGA")]
    GPS_FPGA_ERR,
    #[error("start line or end line out of range, should make them between 0 ~ MaxHeight - 1")]
    GPS_PARAM_OUT_OF_RANGE,
    #[error("GPS has not yet found the satellite or FPGA cannot read GPS data")]
    GPS_DATA_INVALID,
    #[error("Unknown error code")]
    UNKNOWN,
}

fn check_error_code(code: i32) -> Result<(), ASI_ERROR> {
    match code as u32 {
        ASI_ERROR_CODE_ASI_SUCCESS => Ok(()),
        ASI_ERROR_CODE_ASI_ERROR_INVALID_INDEX => Err(ASI_ERROR::INVALID_INDEX), //no camera connected or index value out of boundary
        ASI_ERROR_CODE_ASI_ERROR_INVALID_ID => Err(ASI_ERROR::INVALID_ID),       //invalid ID
        ASI_ERROR_CODE_ASI_ERROR_INVALID_CONTROL_TYPE => Err(ASI_ERROR::INVALID_CONTROL_TYPE), //invalid control type
        ASI_ERROR_CODE_ASI_ERROR_CAMERA_CLOSED => Err(ASI_ERROR::CAMERA_CLOSED), //camera didn't open
        ASI_ERROR_CODE_ASI_ERROR_CAMERA_REMOVED => Err(ASI_ERROR::CAMERA_REMOVED), //failed to find the camera, maybe the camera has been removed
        ASI_ERROR_CODE_ASI_ERROR_INVALID_PATH => Err(ASI_ERROR::INVALID_PATH), //cannot find the path of the file
        ASI_ERROR_CODE_ASI_ERROR_INVALID_FILEFORMAT => Err(ASI_ERROR::INVALID_FILEFORMAT),
        ASI_ERROR_CODE_ASI_ERROR_INVALID_SIZE => Err(ASI_ERROR::INVALID_SIZE), //wrong video format size
        ASI_ERROR_CODE_ASI_ERROR_INVALID_IMGTYPE => Err(ASI_ERROR::INVALID_IMGTYPE), //unsupported image formate
        ASI_ERROR_CODE_ASI_ERROR_OUTOF_BOUNDARY => Err(ASI_ERROR::OUTOF_BOUNDARY), //the startpos is out of boundary
        ASI_ERROR_CODE_ASI_ERROR_TIMEOUT => Err(ASI_ERROR::TIMEOUT),               //timeout
        ASI_ERROR_CODE_ASI_ERROR_INVALID_SEQUENCE => Err(ASI_ERROR::INVALID_SEQUENCE), //stop capture first
        ASI_ERROR_CODE_ASI_ERROR_BUFFER_TOO_SMALL => Err(ASI_ERROR::BUFFER_TOO_SMALL), //buffer size is not big enough
        ASI_ERROR_CODE_ASI_ERROR_VIDEO_MODE_ACTIVE => Err(ASI_ERROR::VIDEO_MODE_ACTIVE),
        ASI_ERROR_CODE_ASI_ERROR_EXPOSURE_IN_PROGRESS => Err(ASI_ERROR::EXPOSURE_IN_PROGRESS),
        ASI_ERROR_CODE_ASI_ERROR_GENERAL_ERROR => Err(ASI_ERROR::GENERAL_ERROR), //general error, eg: value is out of valid range
        ASI_ERROR_CODE_ASI_ERROR_INVALID_MODE => Err(ASI_ERROR::INVALID_MODE), //the current mode is wrong
        ASI_ERROR_CODE_ASI_ERROR_GPS_NOT_SUPPORTED => Err(ASI_ERROR::GPS_NOT_SUPPORTED), //this camera do not support GPS
        ASI_ERROR_CODE_ASI_ERROR_GPS_VER_ERR => Err(ASI_ERROR::GPS_VER_ERR), //the FPGA GPS ver is too low
        ASI_ERROR_CODE_ASI_ERROR_GPS_FPGA_ERR => Err(ASI_ERROR::GPS_FPGA_ERR), //failed to read or write data to FPGA
        ASI_ERROR_CODE_ASI_ERROR_GPS_PARAM_OUT_OF_RANGE => Err(ASI_ERROR::GPS_PARAM_OUT_OF_RANGE), //start line or end line out of range, should make them between 0 ~ MaxHeight - 1
        ASI_ERROR_CODE_ASI_ERROR_GPS_DATA_INVALID => Err(ASI_ERROR::GPS_DATA_INVALID), //GPS has not yet found the satellite or FPGA cannot read GPS data
        _ => Err(ASI_ERROR::UNKNOWN),
    }
}

pub unsafe fn get_num_of_connected_cameras() -> i32 {
    ASIGetNumOfConnectedCameras()
}

pub unsafe fn get_camera_property(
    pASICameraInfo: *mut ASI_CAMERA_INFO,
    iCameraIndex: ::std::os::raw::c_int,
) -> Result<(), ASI_ERROR> {
    check_error_code(ASIGetCameraProperty(pASICameraInfo, iCameraIndex))
}

pub unsafe fn open_camera(iCameraID: ::std::os::raw::c_int) -> Result<(), ASI_ERROR> {
    check_error_code(ASIOpenCamera(iCameraID))
}

pub unsafe fn init_camera(iCameraID: ::std::os::raw::c_int) -> Result<(), ASI_ERROR> {
    check_error_code(ASIInitCamera(iCameraID))
}

#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum IMG_TYPE {
    //Supported Video Format
    RAW8 = ASI_IMG_TYPE_ASI_IMG_RAW8,
    RGB24 = ASI_IMG_TYPE_ASI_IMG_RGB24,
    RAW16 = ASI_IMG_TYPE_ASI_IMG_RAW16,
    Y8 = ASI_IMG_TYPE_ASI_IMG_Y8,
}

pub unsafe fn set_roi_format(
    iCameraID: ::std::os::raw::c_int,
    iWidth: ::std::os::raw::c_int,
    iHeight: ::std::os::raw::c_int,
    iBin: ::std::os::raw::c_int,
    Img_type: IMG_TYPE,
) -> Result<(), ASI_ERROR> {
    check_error_code(ASISetROIFormat(
        iCameraID,
        iWidth,
        iHeight,
        iBin,
        Img_type as i32,
    ))
}

#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CONTROL_TYPE {
    //Control type//
    GAIN = ASI_CONTROL_TYPE_ASI_GAIN,
    EXPOSURE = ASI_CONTROL_TYPE_ASI_EXPOSURE,
    GAMMA = ASI_CONTROL_TYPE_ASI_GAMMA,
    WB_R = ASI_CONTROL_TYPE_ASI_WB_R,
    WB_B = ASI_CONTROL_TYPE_ASI_WB_B,
    OFFSET = ASI_CONTROL_TYPE_ASI_OFFSET,
    BANDWIDTHOVERLOAD = ASI_CONTROL_TYPE_ASI_BANDWIDTHOVERLOAD,
    OVERCLOCK = ASI_CONTROL_TYPE_ASI_OVERCLOCK,
    TEMPERATURE = ASI_CONTROL_TYPE_ASI_TEMPERATURE, // return 10*temperature
    FLIP = ASI_CONTROL_TYPE_ASI_FLIP,
    AUTO_MAX_GAIN = ASI_CONTROL_TYPE_ASI_AUTO_MAX_GAIN,
    AUTO_MAX_EXP = ASI_CONTROL_TYPE_ASI_AUTO_MAX_EXP, //micro second
    AUTO_TARGET_BRIGHTNESS = ASI_CONTROL_TYPE_ASI_AUTO_TARGET_BRIGHTNESS, //target brightness
    HARDWARE_BIN = ASI_CONTROL_TYPE_ASI_HARDWARE_BIN,
    HIGH_SPEED_MODE = ASI_CONTROL_TYPE_ASI_HIGH_SPEED_MODE,
    COOLER_POWER_PERC = ASI_CONTROL_TYPE_ASI_COOLER_POWER_PERC,
    TARGET_TEMP = ASI_CONTROL_TYPE_ASI_TARGET_TEMP, // not need *10
    COOLER_ON = ASI_CONTROL_TYPE_ASI_COOLER_ON,
    MONO_BIN = ASI_CONTROL_TYPE_ASI_MONO_BIN, //lead to less grid at software bin mode for color camera
    FAN_ON = ASI_CONTROL_TYPE_ASI_FAN_ON,
    PATTERN_ADJUST = ASI_CONTROL_TYPE_ASI_PATTERN_ADJUST,
    ANTI_DEW_HEATER = ASI_CONTROL_TYPE_ASI_ANTI_DEW_HEATER,
    FAN_ADJUST = ASI_CONTROL_TYPE_ASI_FAN_ADJUST,
    PWRLED_BRIGNT = ASI_CONTROL_TYPE_ASI_PWRLED_BRIGNT,
    USBHUB_RESET = ASI_CONTROL_TYPE_ASI_USBHUB_RESET,
    GPS_SUPPORT = ASI_CONTROL_TYPE_ASI_GPS_SUPPORT,
    GPS_START_LINE = ASI_CONTROL_TYPE_ASI_GPS_START_LINE,
    GPS_END_LINE = ASI_CONTROL_TYPE_ASI_GPS_END_LINE,
    ROLLING_INTERVAL = ASI_CONTROL_TYPE_ASI_ROLLING_INTERVAL, //microsecond
}

pub unsafe fn set_control_value(
    iCameraID: ::std::os::raw::c_int,
    ControlType: CONTROL_TYPE,
    lValue: ::std::os::raw::c_long,
    bAuto: bool,
) -> Result<(), ASI_ERROR> {
    check_error_code(ASISetControlValue(
        iCameraID,
        ControlType as i32,
        lValue,
        bAuto as i32,
    ))
}

pub unsafe fn get_control_value(
    iCameraID: ::std::os::raw::c_int,
    ControlType: CONTROL_TYPE,
    plValue: *mut ::std::os::raw::c_long,
    pbAuto: *mut ::std::os::raw::c_int,
) -> Result<(), ASI_ERROR> {
    check_error_code(ASIGetControlValue(
        iCameraID,
        ControlType as i32,
        plValue,
        pbAuto,
    ))
}

pub unsafe fn start_exposure(
    iCameraID: ::std::os::raw::c_int,
    bIsDark: bool,
) -> Result<(), ASI_ERROR> {
    unsafe { check_error_code(ASIStartExposure(iCameraID, bIsDark as i32)) }
}

#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EXPOSURE_STATUS {
    EXP_IDLE = ASI_EXPOSURE_STATUS_ASI_EXP_IDLE, //: idle states, you can start exposure now
    EXP_WORKING = ASI_EXPOSURE_STATUS_ASI_EXP_WORKING, //: exposing
    EXP_SUCCESS = ASI_EXPOSURE_STATUS_ASI_EXP_SUCCESS, //: exposure finished and waiting for download
    EXP_FAILED = ASI_EXPOSURE_STATUS_ASI_EXP_FAILED, //:exposure failed, you need to start exposure again
}

pub unsafe fn get_exp_status(
    iCameraID: ::std::os::raw::c_int,
) -> Result<EXPOSURE_STATUS, ASI_ERROR> {
    unsafe {
        let mut status: u32 = 0;
        check_error_code(ASIGetExpStatus(iCameraID, &mut status))?;

        let status = match status {
            ASI_EXPOSURE_STATUS_ASI_EXP_IDLE => EXPOSURE_STATUS::EXP_IDLE,
            ASI_EXPOSURE_STATUS_ASI_EXP_WORKING => EXPOSURE_STATUS::EXP_WORKING,
            ASI_EXPOSURE_STATUS_ASI_EXP_SUCCESS => EXPOSURE_STATUS::EXP_SUCCESS,
            ASI_EXPOSURE_STATUS_ASI_EXP_FAILED => EXPOSURE_STATUS::EXP_FAILED,
            _ => return Err(ASI_ERROR::UNKNOWN),
        };

        Ok(status)
    }
}

pub unsafe fn get_data_after_exp(
    iCameraID: ::std::os::raw::c_int,
    pBuffer: *mut ::std::os::raw::c_uchar,
    lBuffSize: ::std::os::raw::c_long,
) -> Result<(), ASI_ERROR> {
    unsafe { check_error_code(ASIGetDataAfterExp(iCameraID, pBuffer, lBuffSize)) }
}

pub unsafe fn start_video_capture(iCameraID: ::std::os::raw::c_int) -> Result<(), ASI_ERROR> {
    unsafe { check_error_code(ASIStartVideoCapture(iCameraID)) }
}

pub unsafe fn stop_video_capture(iCameraID: ::std::os::raw::c_int) -> Result<(), ASI_ERROR> {
    unsafe { check_error_code(ASIStopVideoCapture(iCameraID)) }
}

pub unsafe fn get_video_data(
    iCameraID: ::std::os::raw::c_int,
    pBuffer: *mut ::std::os::raw::c_uchar,
    lBuffSize: ::std::os::raw::c_long,
    iWaitms: ::std::os::raw::c_int,
) -> Result<(), ASI_ERROR> {
    unsafe { check_error_code(ASIGetVideoData(iCameraID, pBuffer, lBuffSize, iWaitms)) }
}
