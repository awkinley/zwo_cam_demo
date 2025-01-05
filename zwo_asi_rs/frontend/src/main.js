// import init, { handle_message } from "my-crate";

// init();
import { decodeAsync } from "@msgpack/msgpack";
import "./skeleton.css";
import "./normalize.css";
import "./style.css";

let debug_info_elm = document.getElementById("infoContent");

function updateDebugInfo(info) {
  let text = "";
  for (let key in info) {
    text += key + ": " + info[key] + "\n";
  }
  debug_info_elm.innerText = text;
}

let debug_values = {};
function setDebugValues(values) {
  for (let key in values) {
    debug_values[key] = values[key];
  }

  updateDebugInfo(debug_values);
}

// Append log messages
const logContent = document.getElementById("logContent");
function log(message) {
  const newLog = document.createElement("div");
  newLog.textContent = message;
  logContent.appendChild(newLog);

  // Keep only the last 8 log messages
  while (logContent.childNodes.length > 8) {
    logContent.removeChild(logContent.firstChild);
  }

  // Auto-scroll to the bottom
  logContent.scrollTop = logContent.scrollHeight;
}

function fs() {
  var element = document.documentElement;
  if (element.requestFullscreen) element.requestFullscreen();
  else if (element.mozRequestFullScreen) element.mozRequestFullScreen();
  else if (element.webkitRequestFullscreen) element.webkitRequestFullscreen();
  else if (element.msRequestFullscreen) element.msRequestFullscreen();
}

fs();

// const app = document.getElementById("app-html");
// if (app.requestFullscreen) {
//   log("Requesting fullscreen");
//   app.requestFullscreen().catch((err) => {
//     log(
//       `Error attempting to enable fullscreen mode: ${err.message} (${err.name})`
//     );
//   });
// }

function swizzle_bgr(bgrData, buffer) {
  for (let i = 0, j = 0; i < bgrData.length; i += 3, j += 4) {
    buffer[j] = bgrData[i + 2]; // Red
    buffer[j + 1] = bgrData[i + 1]; // Green
    buffer[j + 2] = bgrData[i]; // Blue
    buffer[j + 3] = 255; // Alpha
  }
}

function swizzle_raw(rgbData, buffer) {
  for (let i = 0, j = 0; i < rgbData.length; i += 1, j += 4) {
    buffer[j] = rgbData[i]; // Red
    buffer[j + 1] = rgbData[i]; // Green
    buffer[j + 2] = rgbData[i]; // Blue
    buffer[j + 3] = 255; // Alpha
  }
}

function swizzle_rgb(rgbData, buffer) {
  for (let i = 0, j = 0; i < rgbData.length; i += 3, j += 4) {
    buffer[j] = rgbData[i]; // Red
    buffer[j + 1] = rgbData[i + 1]; // Green
    buffer[j + 2] = rgbData[i + 2]; // Blue
    buffer[j + 3] = 255; // Alpha
  }
}

function update_controls(controls) {
  const inputs = {
    gainInput: "gain",
    exposureInput: "exposure",
    wbrInput: "wb_r",
    wbbInput: "wb_b",
  };
  for (let key in inputs) {
    let elm = document.getElementById(key);
    let value = elm.value;
    if (!value) {
      elm.value = controls[inputs[key]];
    }
  }
}

/** @type {HTMLCanvasElement} */
const canvas = document.getElementById("videoCanvas");
const ctx = canvas.getContext("2d");
function resizeCanvas() {
  // Set canvas width and height to match the window's size in device pixels
  canvas.width = window.innerWidth;
  canvas.height = window.innerHeight;
  // log("Resizing canvas");

  // Optionally scale drawing (e.g., if you want to match CSS size)
  // context.scale(window.devicePixelRatio, window.devicePixelRatio);

  // Clear canvas (optional)
  // context.clearRect(0, 0, canvas.width, canvas.height);
}

// Resize canvas on window resize
window.addEventListener("resize", resizeCanvas);

resizeCanvas();

let imageData = ctx.createImageData(canvas.width, canvas.height);
let buffer = imageData.data;

async function draw_image_data(imageData) {
  let image_bitmap = await window.createImageBitmap(imageData);

  // Get the aspect ratios of the image and canvas
  const imageAspectRatio = image_bitmap.width / image_bitmap.height;
  const canvasAspectRatio = canvas.width / canvas.height;

  // Variables to store the dimensions and position of the image
  let drawWidth, drawHeight, offsetX, offsetY;

  if (imageAspectRatio > canvasAspectRatio) {
    // Image is wider relative to the canvas
    drawWidth = canvas.width;
    drawHeight = canvas.width / imageAspectRatio;
    offsetX = 0;
    offsetY = (canvas.height - drawHeight) / 2; // Center vertically
  } else {
    // Image is taller relative to the canvas
    drawWidth = canvas.height * imageAspectRatio;
    drawHeight = canvas.height;
    offsetX = (canvas.width - drawWidth) / 2; // Center horizontally
    offsetY = 0;
  }

  // Clear the canvas before drawing
  ctx.clearRect(0, 0, canvas.width, canvas.height);

  // Draw the image centered with correct aspect ratio
  ctx.drawImage(image_bitmap, offsetX, offsetY, drawWidth, drawHeight);

  // Clean up the bitmap
  image_bitmap.close();
}

async function handle_message(event) {
  try {
    // Convert Blob to ArrayBuffer
    const data = event.data;

    // Decode the MessagePack data
    const rawData = await decodeAsync(data.stream());
    const type = rawData["type"];
    if (type == "Preview") {
      let controls = rawData["controls"];
      // update_controls(controls);
      setDebugValues(controls);

      let w = rawData["w"];
      let h = rawData["h"];
      let change_size = imageData.width != w || imageData.height != h;
      if (change_size) {
        imageData = ctx.createImageData(w, h);
        buffer = imageData.data;
        setDebugValues({ img_width: w, img_height: h });
      }

      const bgrData = rawData["img"];
      let bytes_per_pix = 3;
      const BGR = 0;
      const RGB = 1;
      const RAW8 = 2;
      if (rawData["pix"] == RAW8) {
        bytes_per_pix = 1;
      }
      if (
        !bgrData ||
        bgrData.length !== imageData.width * imageData.height * bytes_per_pix
      ) {
        console.error("Invalid image data received");
        return;
      }

      // Rearrange BGR to RGBA
      if (rawData["pix"] == BGR) {
        swizzle_bgr(bgrData, buffer);
      } else if (rawData["pix"] == RGB) {
        swizzle_rgb(bgrData, buffer);
      } else if (rawData["pix"] == RAW8) {
        swizzle_raw(bgrData, buffer);
      }

      draw_image_data(imageData);
      // let image_bitmap = await window.createImageBitmap(imageData);
      // ctx.drawImage(image_bitmap, 0, 0, canvas.width, canvas.height);
      setDebugValues({
        canavs_width: canvas.width,
        canvas_height: canvas.height,
      });

      // image_bitmap.close();
    } else if (type == "CaptureStatus") {
      log(
        `Captured ${rawData["captured_frames"]} / ${rawData["total_frames"]} frames`
      );
    }
  } catch (error) {
    console.error("Error processing image data:", error);
  }
}

let ws_url = "/ws";
if (document.URL.includes(":5173")) {
  ws_url = "ws://localhost:3000/ws";
}
const ws = new WebSocket(ws_url);

ws.onopen = () => {
  log("WebSocket connected");

  for (let key in inputs) {
    let elm = document.getElementById(key);
    let value = elm.value;
    if (value) {
      ws.send(inputs[key] + `:${value}`);
    }
  }
};
ws.onmessage = handle_message;

const inputs = {
  gainInput: "SET_GAIN",
  exposureInput: "SET_EXPOSURE",
  wbrInput: "SET_WB_R",
  wbbInput: "SET_WB_B",
};
console.log(inputs);
for (let key in inputs) {
  let elm = document.getElementById(key);
  // console.log(elm);
  elm.oninput = () => {
    let value = elm.value;
    if (value) {
      ws.send(inputs[key] + `:${value}`);
    }
  };
  // let value = elm.value;
  // log(`${key} value = ${value}`);
  // if (value) {
  //   ws.send(inputs[key] + ":" + value);
  // }
}

document.getElementById("switchOutput").onclick = () =>
  ws.send(`SWITCH_OUTPUT:`);
document.getElementById("startCapture").onclick = () =>
  ws.send(`START_CAPTURE:10`);
document.getElementById("fullScreen").onclick = fs;
// Example usage
// debug_values["FPS"] = 60;
// debug_values["Resolution"] = "1920x1080";
// updateDebugInfo(debug_values);

// addLogMessage("Application started.");
// addLogMessage("Captured frame 1.");

// ws.binaryType = "arraybuffer";
// ws.onmessage = (event) => {
//   performance.mark("start-handle-message");
//   handle_message(event.data, imageData.data);
//   performance.mark("stop-handle-message");
//   performance.measure(
//     "duration-handle-message",
//     "start-handle-message",
//     "stop-handle-message"
//   );
//   ctx.putImageData(imageData, 0, 0);
//   performance.mark("put image data");
//   performance.measure(
//     "put-image-data-duration",
//     "stop-handle-message",
//     "put image data"
//   );
//   // console.log("got websocket message");
//   // let blob = event.data;
//   // console.log("blob.type = ", blob.type);
//   // event.data.bytes();

//   // event.data.arrayBuffer().then((data) => {
//   //   // let data = await event.data.arrayBuffer();
//   //   performance.mark("start-handle-message");
//   //   handle_message(data, imageData.data);
//   //   performance.mark("stop-handle-message");
//   //   performance.measure(
//   //     "duration-handle-message",
//   //     "start-handle-message",
//   //     "stop-handle-message"
//   //   );
//   //   ctx.putImageData(imageData, 0, 0);
//   //   performance.mark("put image data");
//   //   performance.measure(
//   //     "put-image-data-duration",
//   //     "stop-handle-message",
//   //     "put image data"
//   //   );
//   // });

//   // let d = imageData.data;
//   // let msg = `js first pixel = [${d[0]}, ${d[1]}, ${d[2]}, ${d[3]}]`;
//   // console.log(msg);
// };

// init().then(() => {
//   // console.log("init wasm-pack");
//   ws.onmessage = (event) => {
//     // console.log("got websocket message");
//     // let blob = event.data;
//     // console.log("blob.type = ", blob.type);
//     // event.data.bytes();
//     event.data.arrayBuffer().then((data) => {
//       // let data = await event.data.arrayBuffer();
//       performance.mark("start-handle-message");
//       handle_message(data, imageData.data);
//       performance.mark("stop-handle-message");
//       performance.measure(
//         "duration-handle-message",
//         "start-handle-message",
//         "stop-handle-message"
//       );
//       ctx.putImageData(imageData, 0, 0);
//       performance.mark("put image data");
//       performance.measure(
//         "put-image-data-duration",
//         "stop-handle-message",
//         "put image data"
//       );
//     });
//     // let d = imageData.data;
//     // let msg = `js first pixel = [${d[0]}, ${d[1]}, ${d[2]}, ${d[3]}]`;
//     // console.log(msg);
//   };
// });
