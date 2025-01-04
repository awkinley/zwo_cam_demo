// import init, { handle_message } from "my-crate";

// init();
import { decode, decodeAsync } from "@msgpack/msgpack";

function swizzle_bgr(bgrData, buffer) {
  for (let i = 0, j = 0; i < bgrData.length; i += 3, j += 4) {
    buffer[j] = bgrData[i + 2]; // Red
    buffer[j + 1] = bgrData[i + 1]; // Green
    buffer[j + 2] = bgrData[i]; // Blue
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

async function handle_message(event) {
  try {
    // Convert Blob to ArrayBuffer
    const data = event.data;

    // Decode the MessagePack data
    performance.mark("start decode msgpack");
    const rawData = await decodeAsync(data.stream());
    const type = rawData["type"];
    if (type == "Preview") {
      let controls = rawData["controls"];
      update_controls(controls);
      // console.log(rawData["controls"]);

      performance.mark("finish decode msgpack");
      performance.measure(
        "decode-duration",
        "start decode msgpack",
        "finish decode msgpack"
      );

      const bgrData = rawData["img"];
      // console.log(bgrData); // Logs the decoded object
      if (!bgrData || bgrData.length !== canvas.width * canvas.height * 3) {
        console.error("Invalid image data received");
        return;
      }

      // Rearrange BGR to RGBA
      performance.mark("start swizzle");
      // console.log(rawData["pix"]);
      const BGR = 0;
      const RGB = 1;
      if (rawData["pix"] == BGR) {
        swizzle_bgr(bgrData, buffer);
      } else if (rawData["pix"] == RGB) {
        swizzle_rgb(bgrData, buffer);
      }
      performance.mark("end swizzle");
      performance.measure("swizzle-duration", "start swizzle", "end swizzle");

      performance.mark("set image data");
      ctx.putImageData(imageData, 0, 0);
      performance.mark("put image data");
      performance.measure(
        "put-image-data-duration",
        "set image data",
        "put image data"
      );
    } else if (type == "CaptureStatus") {
      console.log(
        `Captured ${rawData["captured_frames"]} / ${rawData["total_frames"]} frames`
      );
    }
  } catch (error) {
    console.error("Error processing image data:", error);
  }
}

const canvas = document.getElementById("videoCanvas");
const ctx = canvas.getContext("2d");

const imageData = ctx.createImageData(canvas.width, canvas.height);
const buffer = imageData.data;

const ws = new WebSocket("ws://localhost:3000/ws");

ws.onopen = () => console.log("WebSocket connected");
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
}

document.getElementById("switchOutput").onclick = () =>
  ws.send(`SWITCH_OUTPUT:`);
document.getElementById("startCapture").onclick = () =>
  ws.send(`START_CAPTURE:10`);

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
