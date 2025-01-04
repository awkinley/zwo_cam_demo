use std::time;

use anyhow::Result;
use image::RgbImage;

use opencv::{
    core::{Scalar, Vec3b, CV_8U, CV_8UC3},
    highgui, imgcodecs,
    prelude::*,
};

use tokio::sync::broadcast;
use zwo_asi_rs::{
    asi::{self},
    camera_controller::{CameraController, ClientPacket, ControlMessages, ImagePacket},
    Camera,
};

use axum::{
    body::Bytes,
    extract::{
        ws::{Message, Utf8Bytes, WebSocket, WebSocketUpgrade},
        State,
    },
    http::{HeaderValue, Method},
    response::IntoResponse,
    routing::{any, get_service},
    Router,
};
use axum_extra::{headers, TypedHeader};
use std::net::SocketAddr;
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, TraceLayer},
};

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

//allows to extract the IP of connecting user
use axum::extract::connect_info::ConnectInfo;

//allows to split the websocket stream into separate TX and RX branches
use futures::{sink::SinkExt, stream::StreamExt};

fn get_camera_info() -> impl Iterator<Item = Camera> {
    unsafe {
        // let num_connected = asi::ASIGetNumOfConnectedCameras();
        let num_connected = std::dbg!(asi::get_num_of_connected_cameras());

        (0..num_connected).filter_map(|i| {
            let mut info = asi::ASI_CAMERA_INFO::new();

            let idx: std::os::raw::c_int = i as std::os::raw::c_int;
            asi::get_camera_property(&mut info, idx)
                .map(|_| Camera::new(info))
                .ok()
        })
    }
}

fn test_save_tiff_perf() -> Result<()> {
    let camera = get_camera_info()
        .next()
        .ok_or(anyhow::anyhow!("No camera available."))?;

    println!("Camera: {}", camera.get_name());

    let ccd = camera.open()?;
    ccd.init()?;

    ccd.set_roi_format(1920, 1080, 1, asi::IMG_TYPE::RGB24)?;

    ccd.set_control_value(asi::CONTROL_TYPE::EXPOSURE, 100 * 1000, false)?;
    ccd.set_control_value(asi::CONTROL_TYPE::GAIN, 100, false)?;
    ccd.set_control_value(asi::CONTROL_TYPE::BANDWIDTHOVERLOAD, 50, false)?;

    for _ in 0..10 {
        let mut img = RgbImage::new(1920, 1080);
        ccd.take_exposure(img.as_flat_samples_mut().samples)?;

        let typ = CV_8UC3;
        let mut frame = Mat::new_rows_cols_with_default(1080, 1920, typ, Scalar::all(0.)).unwrap();
        ccd.take_exposure(frame.data_bytes_mut()?)?;

        let start = time::Instant::now();
        img.save_with_format("./image.tiff", image::ImageFormat::Tiff)?;
        let end = time::Instant::now();
        let duration = end - start;
        println!("Saving tiff using image took {:?}", duration);

        let start = time::Instant::now();
        let mut params = opencv::core::Vector::<i32>::new();
        params.push(opencv::imgcodecs::IMWRITE_TIFF_COMPRESSION);
        params.push(1);
        // imgcodecs::imencode("tiff", &frame, buf, params)
        imgcodecs::imwrite("frame.tiff", &frame, &params)?;
        // opencv::
        let end = time::Instant::now();
        let duration = end - start;
        println!("Saving tiff using opencv took {:?}", duration);
    }
    Ok(())
}

// #[derive(Copy, Clone, Debug, PartialEq)]
// enum ControlMessages {
//     SetGain(i32),
//     SetExposure(f32),
//     SetWbR(i32),
//     SetWbB(i32),
//     SwitchOutput,
// }

#[derive(Clone)]
struct WebSocketState {
    tx: broadcast::Sender<ControlMessages>,
    rx: broadcast::Sender<ClientPacket>,
}

// fn make_hist_plot(hist: &ChannelHistogram) -> RgbImage {
//     let mut img = RgbImage::new(1920, 1080);
//     {
//         let drawing_area =
//             BitMapBackend::with_buffer(img.as_flat_samples_mut().samples, (1920, 1080))
//                 .into_drawing_area();

//         drawing_area.fill(&WHITE).unwrap();
//         let max_value = hist
//             .channels
//             .iter()
//             .filter_map(|v| v.iter().max())
//             .max()
//             .unwrap_or(&1);

//         let mut ctx = ChartBuilder::on(&drawing_area)
//             .set_label_area_size(LabelAreaPosition::Left, 40)
//             .set_label_area_size(LabelAreaPosition::Bottom, 40)
//             .build_cartesian_2d(0..255, 0..*max_value)
//             .unwrap();
//         ctx.configure_mesh().draw().unwrap();

//         ctx.draw_series(LineSeries::new(
//             hist.channels[0]
//                 .iter()
//                 .enumerate()
//                 .map(|(idx, v)| (idx as i32, *v as u32))
//                 .collect::<Vec<_>>(),
//             &RED,
//         ))
//         .unwrap();
//         ctx.draw_series(LineSeries::new(
//             hist.channels[1]
//                 .iter()
//                 .enumerate()
//                 .map(|(idx, v)| (idx as i32, *v as u32))
//                 .collect::<Vec<_>>(),
//             &GREEN,
//         ))
//         .unwrap();
//         ctx.draw_series(LineSeries::new(
//             hist.channels[2]
//                 .iter()
//                 .enumerate()
//                 .map(|(idx, v)| (idx as i32, *v as u32))
//                 .collect::<Vec<_>>(),
//             &BLUE,
//         ))
//         .unwrap();
//     }

//     img
// }

// #[derive(Clone, Debug, Serialize_repr)]
// #[repr(u8)]
// enum PixelOrder {
//     BGR = 0,
//     RGB = 1,
// }

// #[derive(Clone, Debug, Serialize)]
// struct ImagePacket {
//     w: u32,
//     h: u32,
//     pix: PixelOrder,
//     #[serde(with = "serde_bytes")]
//     img: Vec<u8>,
// }

// #[derive(Clone, Debug, Serialize)]
// struct ControlValues {
//     gain: i64,
//     exposure: f64,
//     wb_r: i64,
//     wb_b: i64,
// }

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                format!("{}=debug,tower_http=debug", env!("CARGO_CRATE_NAME")).into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Broadcast channel for WebSocket connections
    let (tx, rx) = broadcast::channel(32);
    let (tx_cmds, mut rx_cmds) = broadcast::channel(32);

    let tx_thread = tx.clone();
    let thread = std::thread::spawn(move || -> Result<()> {
        println!("In thread!");
        let camera = get_camera_info()
            .next()
            .ok_or(anyhow::anyhow!("No camera available."))?;
        let mut controller = match CameraController::new(&camera, tx_thread, rx_cmds) {
            Ok(c) => c,
            Err(e) => {
                println!("Initializing CameraController failed with {e}");
                panic!("Initializing CameraController failed with {e}");
            }
        };

        match controller.run() {
            Err(e) => {
                println!("Running CameraController failed with {e}");
                panic!("Running CameraController failed with {e}");
            }
            Ok(r) => Ok(r),
        }
    });
    // if let Ok(res) = thread.join() {
    //     res?;
    // }

    // Define app routes
    let app = Router::new()
        // .route("/ws", get(handle_ws.with_state(tx.clone())))
        .fallback_service(ServeDir::new("web").append_index_html_on_directories(true))
        .route(
            "/ws",
            any(ws_handler).with_state(WebSocketState {
                tx: tx_cmds.clone(),
                rx: tx.clone(),
            }),
        )
        .layer(
            tower::ServiceBuilder::new().layer(
                tower_http::cors::CorsLayer::new()
                    .allow_origin(HeaderValue::from_static("*"))
                    .allow_methods(vec![Method::GET, Method::POST]),
            ),
        )
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(false)),
        );

    // Start the server
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();

    Ok(())
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<WebSocketState>,
) -> impl IntoResponse {
    let user_agent = if let Some(TypedHeader(user_agent)) = user_agent {
        user_agent.to_string()
    } else {
        String::from("Unknown browser")
    };
    println!("`{user_agent}` at {addr} connected.");
    // finalize the upgrade process by returning upgrade callback.
    // we can customize the callback by sending additional info such as address.
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(stream: axum::extract::ws::WebSocket, state: WebSocketState) {
    let (mut sender, mut receiver) = stream.split();
    state.tx.send(ControlMessages::StartPreview).unwrap();

    let mut transmit_rx = state.rx.subscribe();
    // Spawn a task to send broadcasted messages to this client
    let tx_task = tokio::spawn(async move {
        // let mut underlying_bytes = BytesMut::new();
        // let slice: &mut [u8] = &mut underlying_bytes[..];
        // let mut msgpack_data = Rc::<Vec<u8>>::new(vec![]);
        // let mut msgpack_data = Rc::<Vec<u8>>::new(vec![]);
        // let mut serializer = rmp_serde::Serializer::new(msgpack_data)
        //     .with_bytes(rmp_serde::config::BytesMode::ForceAll);
        while let Ok(msg) = transmit_rx.recv().await {
            // {
            //     msg.serialize(&mut serializer).unwrap();
            // }
            // let packet = serializer.get_ref();
            // let buf: &mut Vec<u8> = msgpack_data.as_mut();
            let mut buf = Vec::new();
            // rmp::encode::write_map_len(&mut buf, 1);
            rmp_serde::encode::write_named(&mut buf, &msg).unwrap();

            if sender
                .send(axum::extract::ws::Message::Binary(buf.into()))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    // Handle incoming messages
    while let Some(Ok(msg)) = receiver.next().await {
        if let axum::extract::ws::Message::Text(text) = msg {
            let str = text.as_str();
            if let Some((cmd, val)) = str.split_once(":") {
                let command = match cmd {
                    "SET_GAIN" => ControlMessages::SetGain(val.parse().unwrap()),
                    "SET_EXPOSURE" => ControlMessages::SetExposure(val.parse().unwrap()),
                    "SET_WB_B" => ControlMessages::SetWbB(val.parse().unwrap()),
                    "SET_WB_R" => ControlMessages::SetWbR(val.parse().unwrap()),
                    "SWITCH_OUTPUT" => ControlMessages::SwitchOutput,
                    "START_CAPTURE" => ControlMessages::StartCapture(val.parse().unwrap()),
                    _ => panic!("Unknown command {}", cmd),
                };
                let _ = state.tx.send(command);
            }
        }
    }

    state.tx.send(ControlMessages::StopPreview).unwrap();
    // Drop the broadcast receiver to stop the task
    tx_task.abort();
}
