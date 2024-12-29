#include <opencv2/core/types_c.h>
#include <sys/time.h>

#include <opencv2/opencv.hpp>
#include <thread>
#include <websocketpp/base64/base64.hpp>
#include <websocketpp/config/asio_no_tls.hpp>
#include <websocketpp/server.hpp>

#include "ASICamera2.h"
#include "opencv2/highgui/highgui_c.h"
#include "stdio.h"

typedef websocketpp::server<websocketpp::config::asio> server;
std::mutex image_mutex;
cv::Mat latest_image;

// Converts a cv::Mat to a base64-encoded JPEG string
std::string matToBase64(const cv::Mat& image) {
  std::vector<uchar> buffer;
  cv::imencode(".jpg", image, buffer);
  return websocketpp::base64_encode(buffer.data(), buffer.size());
  // std::string base64_data =
  //     "data:image/jpeg;base64," +
  //     std::string(reinterpret_cast<char*>(buffer.data()), buffer.size());
  // return base64_data;
}

#define MAX_CONTROL 7

class ASICamera {
  ASI_CAMERA_INFO info;

 public:
  ASICamera(ASI_CAMERA_INFO info) : info{info} {};

  void init() {
    bool err = ASIOpenCamera(info.CameraID);
    err += ASIInitCamera(info.CameraID);
    if (err) {
      throw std::runtime_error("Error initializing camera. Are you root?");
    }
  }

  std::vector<ASI_CONTROL_CAPS> getControls() {
    int ctrlnum;
    ASIGetNumOfControls(info.CameraID, &ctrlnum);
    std::vector<ASI_CONTROL_CAPS> controls{};
    controls.resize(ctrlnum);

    for (int i = 0; i < ctrlnum; i++) {
      ASIGetControlCaps(info.CameraID, i, &controls.at(i));
    }

    return controls;
  }

  void printInfo() {
    printf("%s information\n", info.Name);
    int iMaxWidth, iMaxHeight;
    iMaxWidth = info.MaxWidth;
    iMaxHeight = info.MaxHeight;
    printf("resolution:%dX%d\n", iMaxWidth, iMaxHeight);
    const char* bayer[] = {"RG", "BG", "GR", "GB"};
    if (info.IsColorCam)
      printf("Color Camera: bayer pattern:%s\n", bayer[info.BayerPattern]);
    else
      printf("Mono camera\n");

    for (const auto control : getControls()) {
      printf("Control Name: %s\n", control.Name);
      printf("\tdesc: %s\n", control.Description);
      printf("\tmin value: %ld\n", control.MinValue);
      printf("\tmax value: %ld\n", control.MaxValue);
      printf("\tdefault value: %ld\n", control.DefaultValue);
    }
    // int ctrlnum;
    // ASIGetNumOfControls(info.CameraID, &ctrlnum);
    // ASI_CONTROL_CAPS ctrlcap;
    // for (int i = 0; i < ctrlnum; i++) {
    //   ASIGetControlCaps(info.CameraID, i, &ctrlcap);

    //   printf("%s\n", ctrlcap.Name);
    // }
  }

  int maxWidth() { return info.MaxWidth; }

  int maxHeight() { return info.MaxHeight; }
};

void cvText(IplImage* img, const char* text, int x, int y) {
  CvFont font;

  double hscale = 0.6;
  double vscale = 0.6;
  int linewidth = 2;
  cvInitFont(&font, CV_FONT_HERSHEY_SIMPLEX | CV_FONT_ITALIC, hscale, vscale, 0,
             linewidth);

  CvScalar textColor = cvScalar(255, 255, 255);
  CvPoint textPos = cvPoint(x, y);

  cvPutText(img, text, textPos, &font, textColor);
}

extern unsigned long GetTickCount();

int bDisplay = 0;
int bMain = 1;
int bChangeFormat = 0;
int bSendTiggerSignal = 0;
ASI_CAMERA_INFO CamInfo;
enum CHANGE {
  change_imagetype = 0,
  change_bin,
  change_size_bigger,
  change_size_smaller
};
CHANGE change;
void* Display(IplImage* params) {
  IplImage* pImg = params;
  cvNamedWindow("video", 1);
  while (bDisplay) {
    cvShowImage("video", pImg);

    char c = cvWaitKey(1);
    switch (c) {
      case 27:  // esc
        bDisplay = false;
        bMain = false;
        goto END;

      case 'i':  // space
        bChangeFormat = true;
        change = change_imagetype;
        break;

      case 'b':  // space
        bChangeFormat = true;
        change = change_bin;
        break;

      case 'w':  // space
        bChangeFormat = true;
        change = change_size_smaller;
        break;

      case 's':  // space
        bChangeFormat = true;
        change = change_size_bigger;
        break;

      case 't':  // triiger
        bSendTiggerSignal = true;
        break;
    }
  }
END:
  cvDestroyWindow("video");
  printf("Display thread over\n");
  ASIStopVideoCapture(CamInfo.CameraID);
  return (void*)0;
}

class WebSocketServer {
 public:
  WebSocketServer() {
    using namespace std::placeholders;
    ws_server.set_access_channels(websocketpp::log::alevel::none);
    ws_server.init_asio();
    ws_server.set_message_handler(
        std::bind(&WebSocketServer::onMessage, this, _1, _2));
    ws_server.set_open_handler(std::bind(&WebSocketServer::onOpen, this, _1));
    ws_server.set_close_handler(std::bind(&WebSocketServer::onClose, this, _1));
  }

  void run(uint16_t port) {
    ws_server.listen(port);
    ws_server.start_accept();
    ws_server.run();
  }

  void broadcastImage() {
    std::lock_guard<std::mutex> lock(image_mutex);
    if (!latest_image.empty()) {
      std::string base64_img = matToBase64(latest_image);
      for (auto conn : connections) {
        ws_server.send(conn, base64_img, websocketpp::frame::opcode::TEXT);
      }
    }
  }

  void onMessage(websocketpp::connection_hdl hdl, server::message_ptr msg) {
    std::string payload = msg->get_payload();
    if (payload.find("SET_GAIN") != std::string::npos) {
      // Extract and handle gain setting update
      std::cout << "Received gain update: " << payload << std::endl;
    }
  }

  void onOpen(websocketpp::connection_hdl hdl) { connections.insert(hdl); }

  void onClose(websocketpp::connection_hdl hdl) { connections.erase(hdl); }

 private:
  server ws_server;
  std::set<websocketpp::connection_hdl,
           std::owner_less<websocketpp::connection_hdl>>
      connections;
};

// Captures and updates images in a separate thread
void captureImages() {
  // cv::VideoCapture cap(0);  // Use default camera
  // if (!cap.isOpened()) {
  //   std::cerr << "Failed to open camera!" << std::endl;
  //   return;
  // }

  // auto pRgb = cv::CreateMat(cvSize(1920, 1080), IPL_DEPTH_8U, 3);
  cv::Mat frame{cv::Size{1920, 1080}, CV_8UC3, cv::Scalar(0, 0, 0)};
  // frame.create(cv::Size{1920, 1080}, CV_8UC3);

  while (true) {
    ASIStartExposure(CamInfo.CameraID, ASI_FALSE);
    usleep(50000);  // 10ms
    auto status = ASI_EXP_WORKING;
    while (status == ASI_EXP_WORKING) {
      ASIGetExpStatus(CamInfo.CameraID, &status);
    }

    const auto bufSize = frame.total() * frame.elemSize();
    std::cout << "bufSize = " << bufSize << "\n";

    ASIGetDataAfterExp(CamInfo.CameraID, frame.data,
                       frame.total() * frame.elemSize());

    // cv::imwrite("./img.png", frame);
    // cv::Mat frame;
    // cap >> frame;
    // if (frame.empty()) continue;

    std::lock_guard<std::mutex> lock(image_mutex);
    latest_image = frame;
    std::this_thread::sleep_for(
        std::chrono::seconds(1));  // Simulate periodic update
  }
}

std::vector<ASI_CAMERA_INFO> getAvailableCameras() {
  int numDevices = ASIGetNumOfConnectedCameras();
  if (numDevices <= 0) {
    return {};
  }

  printf("attached cameras:\n");
  std::vector<ASI_CAMERA_INFO> cameras{std::size_t(numDevices)};
  cameras.resize(numDevices);
  printf("cameras.size() = %zu\n", cameras.size());

  for (int i = 0; i < numDevices; i++) {
    ASIGetCameraProperty(&cameras.at(i), i);
    printf("%d %s\n", i, cameras[i].Name);
  }

  return cameras;
}

int main() {
  int width;
  const char* bayer[] = {"RG", "BG", "GR", "GB"};
  const char* controls[MAX_CONTROL] = {
      "Exposure", "Gain", "Gamma", "WB_R", "WB_B", "Brightness", "USB Traffic"};

  int height;
  int i;
  char c;
  bool bresult;
  int modeIndex;

  int time1, time2;
  int count = 0;

  char buf[128] = {0};

  int CamIndex = 0;
  int inputformat;
  int definedformat;

  IplImage* pRgb;

  const auto cameras = getAvailableCameras();
  if (cameras.empty()) {
    printf("no camera connected, press any key to exit\n");
    getchar();
    return -1;
  }

  // int numDevices = ASIGetNumOfConnectedCameras();
  // if (numDevices <= 0) {
  //   printf("no camera connected, press any key to exit\n");
  //   getchar();
  //   return -1;
  // } else
  //   printf("attached cameras:\n");

  // for (i = 0; i < numDevices; i++) {
  //   ASIGetCameraProperty(&CamInfo, i);
  //   printf("%d %s\n", i, CamInfo.Name);
  // }

  CamIndex = 0;
  if (cameras.size() > 1) {
    printf("\nselect one to privew\n");
    scanf("%d", &CamIndex);
  }

  // ASI_CAMERA_INFO CamInfo;
  ASIGetCameraProperty(&CamInfo, CamIndex);
  ASICamera camera{CamInfo};

  camera.init();
  // bresult = ASIOpenCamera(CamInfo.CameraID);
  // bresult += ASIInitCamera(CamInfo.CameraID);
  // if (bresult) {
  //   printf("OpenCamera error,are you root?,press any key to exit\n");
  //   getchar();
  //   return -1;
  // }

  camera.printInfo();
  // printf("%s information\n", CamInfo.Name);
  // int iMaxWidth, iMaxHeight;
  // iMaxWidth = CamInfo.MaxWidth;
  // iMaxHeight = CamInfo.MaxHeight;
  // printf("resolution:%dX%d\n", iMaxWidth, iMaxHeight);
  // if (CamInfo.IsColorCam)
  //   printf("Color Camera: bayer pattern:%s\n", bayer[CamInfo.BayerPattern]);
  // else
  //   printf("Mono camera\n");

  // int ctrlnum;
  // ASIGetNumOfControls(CamInfo.CameraID, &ctrlnum);
  // ASI_CONTROL_CAPS ctrlcap;
  // for (i = 0; i < ctrlnum; i++) {
  //   ASIGetControlCaps(CamInfo.CameraID, i, &ctrlcap);

  //   printf("%s\n", ctrlcap.Name);
  // }
  /*
          ASI_SUPPORTED_MODE cammode;
          ASI_CAMERA_MODE mode;
          if(CamInfo.IsTriggerCam)
          {
                  i = 0;
                  printf("This is multi mode camera, you need to select the
     camera mode:\n"); ASIGetCameraSupportMode(CamInfo.CameraID, &cammode);
                  while(cammode.SupportedCameraMode[i]!= ASI_MODE_END)
                  {
                          if(cammode.SupportedCameraMode[i]==ASI_MODE_NORMAL)
                                  printf("%d:Normal Mode\n", i);
                          if(cammode.SupportedCameraMode[i]==ASI_MODE_TRIG_SOFT_EDGE)
                                  printf("%d:Trigger Soft Edge Mode\n", i);
                          if(cammode.SupportedCameraMode[i]==ASI_MODE_TRIG_RISE_EDGE)
                                  printf("%d:Trigger Rise Edge Mode\n", i);
                          if(cammode.SupportedCameraMode[i]==ASI_MODE_TRIG_FALL_EDGE)
                                  printf("%d:Trigger Fall Edge Mode\n", i);
                          if(cammode.SupportedCameraMode[i]==ASI_MODE_TRIG_SOFT_LEVEL)
                                  printf("%d:Trigger Soft Level Mode\n", i);
                          if(cammode.SupportedCameraMode[i]==ASI_MODE_TRIG_HIGH_LEVEL)
                                  printf("%d:Trigger High Level Mode\n", i);
                          if(cammode.SupportedCameraMode[i]==ASI_MODE_TRIG_LOW_LEVEL)
                                  printf("%d:Trigger Low  Lovel Mode\n", i);

                          i++;
                  }

                  scanf("%d", &modeIndex);
                  ASISetCameraMode(CamInfo.CameraID,
     cammode.SupportedCameraMode[modeIndex]); ASIGetCameraMode(CamInfo.CameraID,
     &mode); if(mode != cammode.SupportedCameraMode[modeIndex]) printf("Set mode
     failed!\n");

          }
  */
  int iMaxWidth = camera.maxWidth();
  int iMaxHeight = camera.maxHeight();

  // int bin = 1, Image_type;
  // printf(
  //     "Use customer format or predefined fromat resolution?\n 0:customer "
  //     "format \n 1:predefined format\n");
  // scanf("%d", &inputformat);
  // if (inputformat) {
  //   printf("0:Size %d X %d, BIN 1, ImgType raw8\n", iMaxWidth, iMaxHeight);
  //   printf("1:Size %d X %d, BIN 1, ImgType raw16\n", iMaxWidth, iMaxHeight);
  //   printf("2:Size 1920 X 1080, BIN 1, ImgType raw8\n");
  //   printf("3:Size 1920 X 1080, BIN 1, ImgType raw16\n");
  //   printf("4:Size 320 X 240, BIN 2, ImgType raw8\n");
  //   scanf("%d", &definedformat);
  //   if (definedformat == 0) {
  //     ASISetROIFormat(CamInfo.CameraID, iMaxWidth, iMaxHeight, 1,
  //     ASI_IMG_RAW8); width = iMaxWidth; height = iMaxHeight; bin = 1;
  //     Image_type = ASI_IMG_RAW8;
  //   } else if (definedformat == 1) {
  //     ASISetROIFormat(CamInfo.CameraID, iMaxWidth, iMaxHeight, 1,
  //                     ASI_IMG_RAW16);
  //     width = iMaxWidth;
  //     height = iMaxHeight;
  //     bin = 1;
  //     Image_type = ASI_IMG_RAW16;
  //   } else if (definedformat == 2) {
  //     ASISetROIFormat(CamInfo.CameraID, 1920, 1080, 1, ASI_IMG_RAW8);
  //     width = 1920;
  //     height = 1080;
  //     bin = 1;
  //     Image_type = ASI_IMG_RAW8;
  //   } else if (definedformat == 3) {
  //     ASISetROIFormat(CamInfo.CameraID, 1920, 1080, 1, ASI_IMG_RAW16);
  //     width = 1920;
  //     height = 1080;
  //     bin = 1;
  //     Image_type = ASI_IMG_RAW16;
  //   } else if (definedformat == 4) {
  //     ASISetROIFormat(CamInfo.CameraID, 320, 240, 2, ASI_IMG_RAW8);
  //     width = 320;
  //     height = 240;
  //     bin = 2;
  //     Image_type = ASI_IMG_RAW8;

  //   } else {
  //     printf("Wrong input! Will use the resolution0 as default.\n");
  //     ASISetROIFormat(CamInfo.CameraID, iMaxWidth, iMaxHeight, 1,
  //     ASI_IMG_RAW8); width = iMaxWidth; height = iMaxHeight; bin = 1;
  //     Image_type = ASI_IMG_RAW8;
  //   }

  // } else {
  //   printf(
  //       "\nPlease input the <width height bin image_type> with one space, ie.
  //       " "640 480 2 0. use max resolution if input is 0. Press ESC when
  //       video " "window is focused to quit capture\n");
  //   scanf("%d %d %d %d", &width, &height, &bin, &Image_type);
  //   if (width == 0 || height == 0) {
  //     width = iMaxWidth;
  //     height = iMaxHeight;
  //   }

  //   while (ASISetROIFormat(CamInfo.CameraID, width, height, bin,
  //                          (ASI_IMG_TYPE)Image_type))  // IMG_RAW8
  //   {
  //     printf(
  //         "Set format error, please check the width and height\n ASI120's
  //         data " "size(width*height) must be integer multiple of 1024\n");
  //     printf("Please input the width and height again, ie. 640 480\n");
  //     scanf("%d %d %d %d", &width, &height, &bin, &Image_type);
  //   }
  //   printf(
  //       "\nset image format %d %d %d %d success, start privew, press ESC to "
  //       "stop \n",
  //       width, height, bin, Image_type);
  // }

  // if (Image_type == ASI_IMG_RAW16)
  //   pRgb = cvCreateImage(cvSize(width, height), IPL_DEPTH_16U, 1);
  // else if (Image_type == ASI_IMG_RGB24)
  //   pRgb = cvCreateImage(cvSize(width, height), IPL_DEPTH_8U, 3);
  // else
  //   pRgb = cvCreateImage(cvSize(width, height), IPL_DEPTH_8U, 1);

  ASISetROIFormat(CamInfo.CameraID, iMaxWidth, iMaxHeight, 1, ASI_IMG_RGB24);

  int exp_ms{50};
  // printf("Please input exposure time(ms)\n");
  // scanf("%d", &exp_ms);
  ASISetControlValue(CamInfo.CameraID, ASI_EXPOSURE, exp_ms * 1000, ASI_FALSE);
  // ASISetControlValue(CamInfo.CameraID,ASI_GAIN,0, ASI_FALSE);
  ASISetControlValue(CamInfo.CameraID, ASI_GAIN, 255, ASI_TRUE);
  ASISetControlValue(CamInfo.CameraID, ASI_BANDWIDTHOVERLOAD, 40,
                     ASI_FALSE);  // low transfer speed
  ASISetControlValue(CamInfo.CameraID, ASI_HIGH_SPEED_MODE, 0, ASI_FALSE);
  ASISetControlValue(CamInfo.CameraID, ASI_WB_B, 90, ASI_FALSE);
  ASISetControlValue(CamInfo.CameraID, ASI_WB_R, 48, ASI_TRUE);

  WebSocketServer ws_server;
  std::thread ws_thread([&]() { ws_server.run(9002); });

  std::thread capture_thread(captureImages);

  while (true) {
    ws_server.broadcastImage();
    std::this_thread::sleep_for(std::chrono::seconds(2));
  }

  ws_thread.join();
  capture_thread.join();

  //   ASI_SUPPORTED_MODE cammode;
  //   ASI_CAMERA_MODE mode;
  //   if (CamInfo.IsTriggerCam) {
  //     i = 0;
  //     printf("This is multi mode camera, you need to select the camera
  //     mode:\n"); ASIGetCameraSupportMode(CamInfo.CameraID, &cammode); while
  //     (cammode.SupportedCameraMode[i] != ASI_MODE_END) {
  //       if (cammode.SupportedCameraMode[i] == ASI_MODE_NORMAL)
  //         printf("%d:Normal Mode\n", i);
  //       if (cammode.SupportedCameraMode[i] == ASI_MODE_TRIG_SOFT_EDGE)
  //         printf("%d:Trigger Soft Edge Mode\n", i);
  //       if (cammode.SupportedCameraMode[i] == ASI_MODE_TRIG_RISE_EDGE)
  //         printf("%d:Trigger Rise Edge Mode\n", i);
  //       if (cammode.SupportedCameraMode[i] == ASI_MODE_TRIG_FALL_EDGE)
  //         printf("%d:Trigger Fall Edge Mode\n", i);
  //       if (cammode.SupportedCameraMode[i] == ASI_MODE_TRIG_SOFT_LEVEL)
  //         printf("%d:Trigger Soft Level Mode\n", i);
  //       if (cammode.SupportedCameraMode[i] == ASI_MODE_TRIG_HIGH_LEVEL)
  //         printf("%d:Trigger High Level Mode\n", i);
  //       if (cammode.SupportedCameraMode[i] == ASI_MODE_TRIG_LOW_LEVEL)
  //         printf("%d:Trigger Low  Lovel Mode\n", i);

  //       i++;
  //     }

  //     scanf("%d", &modeIndex);
  //     ASISetCameraMode(CamInfo.CameraID,
  //     cammode.SupportedCameraMode[modeIndex]);
  //     ASIGetCameraMode(CamInfo.CameraID, &mode);
  //     if (mode != cammode.SupportedCameraMode[modeIndex])
  //       printf("Set mode failed!\n");
  //   }

  //   ASIStartVideoCapture(CamInfo.CameraID);  // start privew

  //   long lVal;
  //   ASI_BOOL bAuto;
  //   ASIGetControlValue(CamInfo.CameraID, ASI_TEMPERATURE, &lVal, &bAuto);
  //   printf("sensor temperature:%.1f\n", lVal / 10.0);

  //   bDisplay = 1;
  //   std::thread thread_display{Display, pRgb};

  //   time1 = GetTickCount();
  //   int iStrLen = 0, iTextX = 40, iTextY = 60;
  //   void* retval;

  //   int iDropFrmae;
  //   while (bMain) {
  //     if (mode == ASI_MODE_NORMAL) {
  //       if (ASIGetVideoData(CamInfo.CameraID, (unsigned
  //       char*)pRgb->imageData,
  //                           pRgb->imageSize, 500) == ASI_SUCCESS)
  //         count++;
  //     } else {
  //       if (ASIGetVideoData(CamInfo.CameraID, (unsigned
  //       char*)pRgb->imageData,
  //                           pRgb->imageSize, 1000) == ASI_SUCCESS)
  //         count++;
  //     }

  //     time2 = GetTickCount();

  //     if (time2 - time1 > 1000) {
  //       ASIGetDroppedFrames(CamInfo.CameraID, &iDropFrmae);
  //       sprintf(buf, "fps:%d dropped frames:%d ImageType:%d", count,
  //       iDropFrmae,
  //               (int)Image_type);

  //       count = 0;
  //       time1 = GetTickCount();
  //       printf("%s", buf);
  //       printf("\n");
  //     }
  //     if (Image_type != ASI_IMG_RGB24 && Image_type != ASI_IMG_RAW16) {
  //       iStrLen = strlen(buf);
  //       CvRect rect = cvRect(iTextX, iTextY - 15, iStrLen * 11, 20);
  //       cvSetImageROI(pRgb, rect);
  //       cvSet(pRgb, cvScalar(180, 180, 180));
  //       cvResetImageROI(pRgb);
  //     }
  //     cvText(pRgb, buf, iTextX, iTextY);

  //     if (bSendTiggerSignal) {
  //       ASISendSoftTrigger(CamInfo.CameraID, ASI_TRUE);
  //       bSendTiggerSignal = 0;
  //     }

  //     if (bChangeFormat) {
  //       bChangeFormat = 0;
  //       bDisplay = false;
  //       thread_display.join();
  //       cvReleaseImage(&pRgb);
  //       ASIStopVideoCapture(CamInfo.CameraID);

  //       switch (change) {
  //         case change_imagetype:
  //           Image_type++;
  //           if (Image_type > 3) Image_type = 0;

  //           break;
  //         case change_bin:
  //           if (bin == 1) {
  //             bin = 2;
  //             width /= 2;
  //             height /= 2;
  //           } else {
  //             bin = 1;
  //             width *= 2;
  //             height *= 2;
  //           }
  //           break;
  //         case change_size_smaller:
  //           if (width > 320 && height > 240) {
  //             width /= 2;
  //             height /= 2;
  //           }
  //           break;

  //         case change_size_bigger:

  //           if (width * 2 * bin <= iMaxWidth && height * 2 * bin <=
  //           iMaxHeight) {
  //             width *= 2;
  //             height *= 2;
  //           }
  //           break;
  //       }
  //       ASISetROIFormat(CamInfo.CameraID, width, height, bin,
  //                       (ASI_IMG_TYPE)Image_type);
  //       if (Image_type == ASI_IMG_RAW16)
  //         pRgb = cvCreateImage(cvSize(width, height), IPL_DEPTH_16U, 1);
  //       else if (Image_type == ASI_IMG_RGB24)
  //         pRgb = cvCreateImage(cvSize(width, height), IPL_DEPTH_8U, 3);
  //       else
  //         pRgb = cvCreateImage(cvSize(width, height), IPL_DEPTH_8U, 1);
  //       bDisplay = 1;
  //       thread_display = std::thread{Display, pRgb};
  //       ASIStartVideoCapture(CamInfo.CameraID);  // start privew
  //     }
  //   }
  // END:

  //   if (bDisplay) {
  //     bDisplay = 0;
  //     thread_display.join();
  //   }

  ASIStopVideoCapture(CamInfo.CameraID);
  ASICloseCamera(CamInfo.CameraID);
  cvReleaseImage(&pRgb);
  printf("main function over\n");
  return 1;
}
