#include <App.h>
#include <opencv2/core/types_c.h>
#include <sys/time.h>

#include <atomic>
#include <opencv2/opencv.hpp>
#include <thread>

#include "ASICamera2.h"
#include "opencv2/highgui/highgui_c.h"
#include "stdio.h"


// typedef websocketpp::server<websocketpp::config::asio> server;
std::mutex image_mutex;
cv::Mat latest_image;
std::atomic_bool hasNewImage;

template <typename T>
class SetValueQueue {
  std::atomic<T> newValue;
  std::atomic_bool hasNewValue;

 public:
  void set(T val) {
    newValue = val;
    hasNewValue = true;
  }

  bool didChange() { return hasNewValue; }

  T get() {
    hasNewValue = false;
    return newValue;
  }
};

SetValueQueue<int> gainValue;

std::string matToBase64(const cv::Mat& image) {
  std::vector<uchar> buffer;
  cv::imencode(".jpg", image, buffer);
  static const char encodeTable[] =
      "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  std::string base64;
  int val = 0, valb = -6;
  for (uchar c : buffer) {
    val = (val << 8) + c;
    valb += 8;
    while (valb >= 0) {
      base64.push_back(encodeTable[(val >> valb) & 0x3F]);
      valb -= 6;
    }
  }
  if (valb > -6)
    base64.push_back(encodeTable[((val << 8) >> (valb + 8)) & 0x3F]);
  while (base64.size() % 4) base64.push_back('=');
  // return "data:image/jpeg;base64," + base64;
  return base64;
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
  }

  int maxWidth() { return info.MaxWidth; }

  int maxHeight() { return info.MaxHeight; }

  void startVideoCapture() { ASIStartVideoCapture(info.CameraID); }

  void stopVideoCapture() { ASIStopVideoCapture(info.CameraID); }

  int getVideoData(unsigned char* data, long bufSize, int waitMs) {
    return ASIGetVideoData(info.CameraID, data, bufSize, waitMs);
  }

  void setControlValue(ASI_CONTROL_TYPE ControlType, long lValue,
                       ASI_BOOL bAuto) {
    ASISetControlValue(info.CameraID, ControlType, lValue, bAuto);
  }

  long getControlValue(ASI_CONTROL_TYPE ControlType) {
    long val;
    ASI_BOOL bAuto;
    ASIGetControlValue(info.CameraID, ControlType, &val, &bAuto);
    return val;
  }
};

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

// Captures and updates images in a separate thread
void captureImages(ASICamera camera) {
  // ASIStartVideoCapture(camera);  // start privew
  camera.startVideoCapture();
  // auto pRgb = cv::CreateMat(cvSize(1920, 1080), IPL_DEPTH_8U, 3);
  cv::Mat frame{cv::Size{1920, 1080}, CV_8UC3, cv::Scalar(0, 0, 0)};
  const auto bufSize = frame.total() * frame.elemSize();
  std::cout << "bufSize = " << bufSize << "\n";
  // frame.create(cv::Size{1920, 1080}, CV_8UC3);
  auto start = std::chrono::system_clock::now();

  while (true) {
    if (camera.getVideoData(frame.data, bufSize, 100) != ASI_SUCCESS) {
      std::cout << "Failed to get video data!\n";
      continue;
    }

    if (gainValue.didChange()) {
      camera.setControlValue(ASI_GAIN, gainValue.get(), ASI_FALSE);
    }

    auto now = std::chrono::system_clock::now();
    const auto msSinceLast =
        std::chrono::duration_cast<std::chrono::milliseconds>(now - start)
            .count();
    if (msSinceLast > 1000) {
      {
        std::lock_guard<std::mutex> lock(image_mutex);
        latest_image = frame;
      }
      hasNewImage.store(true);
      start = now;
    }
  }

  camera.stopVideoCapture();
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
uWS::App* globalApp;

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

  CamIndex = 0;
  if (cameras.size() > 1) {
    printf("\nselect one to privew\n");
    scanf("%d", &CamIndex);
  }

  // ASI_CAMERA_INFO CamInfo;
  ASIGetCameraProperty(&CamInfo, CamIndex);
  ASICamera camera{CamInfo};

  camera.init();

  camera.printInfo();

  int iMaxWidth = camera.maxWidth();
  int iMaxHeight = camera.maxHeight();

  ASISetROIFormat(CamInfo.CameraID, iMaxWidth, iMaxHeight, 1, ASI_IMG_RGB24);

  int exp_ms{50};
  // printf("Please input exposure time(ms)\n");
  // scanf("%d", &exp_ms);
  ASISetControlValue(CamInfo.CameraID, ASI_EXPOSURE, exp_ms * 1000, ASI_FALSE);
  // ASISetControlValue(CamInfo.CameraID,ASI_GAIN,0, ASI_FALSE);
  ASISetControlValue(CamInfo.CameraID, ASI_GAIN, 400, ASI_TRUE);
  ASISetControlValue(CamInfo.CameraID, ASI_BANDWIDTHOVERLOAD, 40,
                     ASI_FALSE);  // low transfer speed
  ASISetControlValue(CamInfo.CameraID, ASI_HIGH_SPEED_MODE, 0, ASI_FALSE);
  ASISetControlValue(CamInfo.CameraID, ASI_WB_B, 90, ASI_FALSE);
  ASISetControlValue(CamInfo.CameraID, ASI_WB_R, 48, ASI_TRUE);

  // std::vector<uWS::WebSocket<false, true>*> clients;

  std::thread capture_thread(captureImages, camera);
  struct PerSocketData {
    /* Fill with user data */
  };

  uWS::App app =
      uWS::App()
          .ws<PerSocketData>(
              "/*",
              {.open =
                   [](auto* ws) {
                     ws->subscribe("images");
                     //  clients.push_back(ws);
                     std::cout << "Client connected." << std::endl;
                   },
               .message =
                   [](auto* ws, std::string_view message, uWS::OpCode opCode) {
                     std::string msg(message);
                     if (msg.find("SET_GAIN") != std::string::npos) {
                       std::cout << "Received gain update: " << msg
                                 << std::endl;
                       std::string floatPart{
                           msg.begin() + std::string("SET_GAIN:").size(),
                           msg.end()};
                       gainValue.set(std::stol(floatPart));
                     }
                   },
               .close =
                   [](auto* ws, int, std::string_view) {
                     std::cout << "Client disconnected." << std::endl;
                   }})
          .listen(9002, [](auto* token) {
            if (token) {
              std::cout << "Server started on port 9002" << std::endl;
            } else {
              std::cerr << "Failed to start server!" << std::endl;
            }
          });

  struct us_loop_t* loop = (struct us_loop_t*)uWS::Loop::get();
  struct us_timer_t* delayTimer = us_create_timer(loop, 0, 0);

  // broadcast the unix time as millis every 8 millis
  us_timer_set(
      delayTimer,
      [](struct us_timer_t* /*t*/) {
        struct timespec ts;
        timespec_get(&ts, TIME_UTC);

        int64_t millis = ts.tv_sec * 1000 + ts.tv_nsec / 1000000;

        if (!hasNewImage) {
          return;
        }
        // std::cout << "Broadcasting timestamp: " << millis << std::endl;
        std::string base64_img;
        {
          std::lock_guard<std::mutex> lock(image_mutex);
          if (!latest_image.empty()) {
            base64_img = matToBase64(latest_image);
          }
          hasNewImage = false;
        }
        globalApp->publish("images", base64_img, uWS::OpCode::TEXT);

        // app->publish("broadcast", std::string_view((char *) &millis,
        // sizeof(millis)), uWS::OpCode::BINARY, false);
      },
      8, 10);

  globalApp = &app;

  app.run();

  capture_thread.join();

  ASIStopVideoCapture(CamInfo.CameraID);
  ASICloseCamera(CamInfo.CameraID);
  cvReleaseImage(&pRgb);
  printf("main function over\n");
  return 0;
}
