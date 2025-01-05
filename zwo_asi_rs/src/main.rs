use anyhow::Result;

use tokio::sync::broadcast;
use zwo_asi_rs::{
    asi::{self},
    camera_controller::{ControlValues, CameraController, ClientPacket, ControlMessages, ImagePacket, PixelOrder},
    Camera,
};

use axum::{
    extract::{ws::WebSocketUpgrade, State},
    http::{HeaderValue, Method},
    response::IntoResponse,
    routing::any,
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
                .map(|()| Camera::new(info))
                .ok()
        })
    }
}

#[derive(Clone)]
struct WebSocketState {
    tx: broadcast::Sender<ControlMessages>,
    rx: broadcast::Sender<ClientPacket>,
}

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
    let (tx, _) = broadcast::channel(32);
    let (tx_cmds, rx_cmds) = broadcast::channel(32);

    let tx_thread = tx.clone();
    let _thread = std::thread::spawn(move || -> Result<()> {
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
        .fallback_service(ServeDir::new("frontend/dist").append_index_html_on_directories(true))
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
    let listener = tokio::net::TcpListener::bind("0.0.0.0:80")
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

        //let controls = ControlValues {
        //    gain: 0,
        //    exposure: 0.,
        //    wb_b: 0,
        //    wb_r: 0,
        //};
        //
        //let packet = ClientPacket::Preview(ImagePacket {
        //    w: 1920,
        //    h: 1080,
        //    pix: PixelOrder::BGR,
        //    img: vec![100; 1920 * 1080 * 3],
        //    controls: controls,
        //});
        //
        //let mut buf = Vec::new();
        //// rmp::encode::write_map_len(&mut buf, 1);
        //rmp_serde::encode::write_named(&mut buf, &packet).unwrap();
        //let bytes: axum::body::Bytes = buf.into();
        //let num_bytes = bytes.len();
        //let ws_message = axum::extract::ws::Message::Binary(bytes);
        //loop {
        //    let send_start = std::time::Instant::now();
        //    if sender
        //        .send(ws_message.clone())
        //        .await
        //        .is_err()
        //    {
        //        println!("Sending message failed");
        //        // break;
        //    }
        //    let send_end = std::time::Instant::now();
        //    println!("Sent messsage in {:?}", send_end - send_start);
        //
        //}
        //return;

        // let mut underlying_bytes = BytesMut::new();
        // let slice: &mut [u8] = &mut underlying_bytes[..];
        // let mut msgpack_data = Rc::<Vec<u8>>::new(vec![]);
        // let mut msgpack_data = Rc::<Vec<u8>>::new(vec![]);
        // let mut serializer = rmp_serde::Serializer::new(msgpack_data)
        //     .with_bytes(rmp_serde::config::BytesMode::ForceAll);
        loop {
            match transmit_rx.recv().await {
                Ok(msg) => {
                    println!("Recived a msg to trasnmit");
                    // {
                    //     msg.serialize(&mut serializer).unwrap();
                    // }
                    // let packet = serializer.get_ref();
                     //let buf: &mut Vec<u8> = msgpack_data.as_mut();
                    //let start = std::time::Instant::now();
                    let mut buf = Vec::new();
                    // rmp::encode::write_map_len(&mut buf, 1);
                    rmp_serde::encode::write_named(&mut buf, &msg).unwrap();
                    //let middle = std::time::Instant::now();
                    //println!("Encoded message in {:?}", middle - start);
                    let bytes: axum::body::Bytes = buf.into();
                    let num_bytes = bytes.len();

                    let ws_message = axum::extract::ws::Message::Binary(bytes);
                    //let end = std::time::Instant::now();
                    //println!("Generated ws_message in {:?}, size = {}KB", end - middle, num_bytes / 1024);

                    let send_start = std::time::Instant::now();
                    if sender
                        .send(ws_message)
                        .await
                        .is_err()
                    {
                        println!("Sending message failed");
                        // break;
                    }
                    let send_end = std::time::Instant::now();
                    println!("Sent messsage in {:?}", send_end - send_start);
                },
                Err(e) => {
                    println!("Getting msg to transmit failed with err {:?}", e);
                    break;
                }
            }
        }
        // while let Ok(msg) = transmit_rx.recv().await {

        //     println!("Recived a msg to trasnmit");
        //     // {
        //     //     msg.serialize(&mut serializer).unwrap();
        //     // }
        //     // let packet = serializer.get_ref();
        //     // let buf: &mut Vec<u8> = msgpack_data.as_mut();
        //     let mut buf = Vec::new();
        //     // rmp::encode::write_map_len(&mut buf, 1);
        //     rmp_serde::encode::write_named(&mut buf, &msg).unwrap();
        //     println!("Encoded message");

        //     if sender
        //         .send(axum::extract::ws::Message::Binary(buf.into()))
        //         .await
        //         .is_err()
        //     {
        //         println!("Sending message failed");
        //         break;
        //     }
        //         println!("Sent messsage");
        // }
        println!("Got here????");
    });

    // Handle incoming messages
    while let Some(Ok(msg)) = receiver.next().await {
        if let axum::extract::ws::Message::Text(text) = msg {
            let str = text.as_str();
            if let Some((cmd, val)) = str.split_once(':') {
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
