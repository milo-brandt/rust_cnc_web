#![allow(dead_code)]

mod cnc;
mod util;

use {
    async_stream::stream,
    axum::{
        extract::{
            ws::{Message, WebSocket, WebSocketUpgrade},
            Json, RawBody,
        },
        response::Response,
        routing::{get, post},
        Extension, Router,
    },
    cnc::{
        broker::{Broker, JobInnerHandle, MachineHandle, StreamJob},
        gcode::{
            parser::{
                parse_gcode_line, parse_generalized_line, GCodeParseError, GeneralizedLine,
                GeneralizedLineOwned,
            },
            GCodeFormatSpecification,
        },
        grbl::new_machine::{
            start_machine, ImmediateRequest, MachineDebugEvent, MachineInterface, WriteRequest,
        },
    },
    futures::{
        sink::SinkExt,
        stream::{SplitStream, StreamExt},
    },
    itertools::Itertools,
    serde::Deserialize,
    std::{str::from_utf8_unchecked, sync::Arc, time::Duration},
    tokio::{
        fs::File,
        io::{AsyncBufReadExt, BufReader},
        join, select,
        sync::oneshot,
        time::sleep,
    },
    tower_http::cors::{Any, CorsLayer},
};

#[tokio::main]
async fn main() {
    let cors = CorsLayer::new()
        // allow `GET` and `POST` when accessing the resource
        .allow_methods(Any)
        // allow requests from any origin... should maybe read in config
        .allow_origin(Any);
    // We should probably add some other authentication?
    // Maybe a header-to-cookie sort of deal?
    // Or double submit cookie?
    let (reader, writer) =
        cnc::connection::open_and_reset_arduino_like_serial("/dev/ttyUSB0").await;
    let machine = start_machine(reader, writer).await.unwrap();
    // build our application with a single route
    let app = Router::new()
        .route("/job/run_file", post(run_gcode_file))
        .route("/debug/send", post(index))
        .route("/debug/gcode_job", post(run_gcode))
        .route("/debug/gcode_unchecked_if_free", post(run_gcode_unchecked))
        .route("/debug/listen_raw", get(listen_raw))
        .route("/debug/listen_status", get(listen_status))
        //.route("/ws", get(websocket_upgrade))
        .layer(cors)
        .layer(Extension(Arc::new(machine)))
        .layer(Extension(Arc::new(Broker::new())));

    // run it with hyper on localhost:3000
    println!("Listening on port 3000...");
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

/* async fn listen_raw(machine: Extension<Arc<Machine>>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    println!("Request to listen!");
    let mut input_receiver = machine.raw_input_subscribe();
    let mut output_receiver = machine.raw_output_subscribe();
    let result = stream! {
        loop {
            let (is_output, value) = select! {
                input = input_receiver.recv() => (false, input),
                output = output_receiver.recv() => (true, output)
            };
            if let Ok(string) = value {
                let prefix = if is_output { "> " } else { "< " };
                yield Event::default().data(format!("{}{}", prefix, string));
            }
        }
    }.map(Ok);

    Sse::new(result).keep_alive(KeepAlive::default())
} */

async fn listen_raw(ws: WebSocketUpgrade, machine: Extension<Arc<MachineInterface>>) -> Response {
    let mut debug_receiver = machine.debug_stream.subscribe_with_history_count(100);
    ws.on_upgrade(move |socket| async move {
        let (mut writer, mut reader) = socket.split();
        let (closer, mut close_listen) = oneshot::channel::<()>();
        let (writer, reader) = join! {
            async move {
                loop {
                    select! {
                        event = debug_receiver.recv() => {
                            let event = event.unwrap();
                            let message = match event {
                                MachineDebugEvent::Sent(str) => format!("> {}", unsafe { from_utf8_unchecked(&str) }),
                                MachineDebugEvent::Received(str) => format!("< {}", str),
                                MachineDebugEvent::Warning(str) => format!("! {}", str),
                                MachineDebugEvent::Comment(str) => format!("~ {}", str),
                            };
                            if writer.send(Message::Text(message)).await.is_err() {
                                break
                            }
                        }
                        _ = &mut close_listen => break
                    }
                }
                writer
            },
            async move {
                //ensure that this is actually getting read - so we can handle close frame!
                loop {
                    let response = <SplitStream<WebSocket> as StreamExt>::next(&mut reader).await;
                    if let Some(Ok(Message::Close(_))) = response {
                        closer.send(()).unwrap();
                        break
                    }
                    if response.is_none() {
                        break
                    }
                }
                reader
            }
        };
        let together = reader.reunite(writer).unwrap();
        drop(together.close().await);
    })
}

fn default_settings() -> GCodeFormatSpecification {
    GCodeFormatSpecification {
        axis_letters: b"XYZA".to_vec(),
        offset_axis_letters: b"IJK".to_vec(),
        float_digits: 3,
    }
}

async fn index(message: RawBody, machine: Extension<Arc<MachineInterface>>) -> String {
    let mut body_bytes = hyper::body::to_bytes(message.0).await.unwrap().to_vec();
    if body_bytes.len() == 1 && body_bytes[0] == b'?' {
        let (sender, receiver) = oneshot::channel();
        machine
            .immediate_write_stream
            .send(ImmediateRequest::Status { result: sender })
            .await
            .unwrap();
        match receiver.await {
            Ok(result) => format!("{:?}", result),
            Err(_) => "Internal error immediate?".to_string(),
        }
    } else {
        let bytes = if !body_bytes.is_empty() && body_bytes[0] == b'\\' {
            body_bytes.push(b'\n');
            body_bytes[1..].to_vec()
        } else {
            let line = unsafe { from_utf8_unchecked(&body_bytes) };
            let gcode = parse_gcode_line(&default_settings(), line);
            match gcode {
                Ok(gcode_line) => format!("{}\n", default_settings().format_line(&gcode_line)),
                Err(err) => return format!("Bad gcode: {:?}", err),
            }
            .into_bytes()
        };
        //body_bytes.push(b'\n');

        //let result = format!("Sent message: {}", from_utf8(&body_bytes).unwrap());
        let (sender, receiver) = oneshot::channel();
        machine
            .write_stream
            .send(WriteRequest::Plain {
                data: bytes,
                result: sender,
            })
            .await
            .unwrap();
        match receiver.await {
            Ok(Ok(())) => "Success!".to_string(),
            Ok(Err(id)) => format!("Failed: {}", id),
            Err(_) => "Internal error?".to_string(),
        }
    }
}
async fn run_gcode(
    message: RawBody,
    machine: Extension<Arc<MachineInterface>>,
    broker: Extension<Arc<Broker>>,
) -> String {
    let body_bytes = hyper::body::to_bytes(message.0).await.unwrap();
    let body = std::str::from_utf8(&body_bytes).unwrap();
    let spec = default_settings();
    let lines: Result<Vec<GeneralizedLineOwned>, (usize, GCodeParseError)> = body
        .lines()
        .enumerate()
        .map(|(index, line)| {
            parse_generalized_line(&spec, line)
                .map(GeneralizedLine::into_owned)
                .map_err(|e| (index, e))
        })
        .collect();
    let lines = match lines {
        Ok(lines) => lines,
        Err((error_index, error)) => {
            return format!("Error on line {} of input!\n{:?}\n", error_index + 1, error)
        }
    };
    let total_lines = lines.len();
    let result = broker.try_send_job(
        StreamJob::new(
            stream! {
                for line in lines.into_iter() {
                    yield line;
                }
            },
            total_lines,
        ),
        MachineHandle {
            write_stream: machine.write_stream.clone(),
            immediate_write_stream: machine.immediate_write_stream.clone(),
        },
    );
    match result {
        Ok(()) => "Job sent!".to_string(),
        Err(_) => "Job not sent!".to_string(),
    }
}
async fn run_gcode_unchecked(
    // Runs the line *if* no job is scheduled yet.
    message: RawBody,
    machine: Extension<Arc<MachineInterface>>,
    broker: Extension<Arc<Broker>>,
) -> String {
    let mut body_bytes = hyper::body::to_bytes(message.0).await.unwrap().to_vec();
    body_bytes.push(b'\n');
    let result = broker.try_send_job(
        move |handle: MachineHandle, _job_handle: JobInnerHandle| async move {
            handle
                .write_stream
                .send(WriteRequest::Plain {
                    data: body_bytes,
                    result: oneshot::channel().0,
                })
                .await
                .unwrap();
        },
        MachineHandle {
            write_stream: machine.write_stream.clone(),
            immediate_write_stream: machine.immediate_write_stream.clone(),
        },
    );
    match result {
        Ok(()) => "Job sent!".to_string(),
        Err(_) => "Job not sent!".to_string(),
    }
}

#[derive(Deserialize)]
struct RunGcodeFile {
    path: String,
}
async fn run_gcode_file(
    message: Json<RunGcodeFile>,
    broker: Extension<Arc<Broker>>,
    machine: Extension<Arc<MachineInterface>>,
) -> String {
    if !message.path.chars().all(|c|
        c.is_ascii_alphanumeric()
        || c == '_'
    ) {
        return "Illegal path!".to_string();
    }
    let spec = default_settings();
    let mut line_count = 0;
    {
        let file = match File::open(format!("gcode/{}", message.path)).await {
            Ok(file) => file,
            Err(e) => return format!("Error! {:?}", e),
        };
        let file = BufReader::new(file);
        let mut lines = file.lines();
        let mut errors = Vec::new();
        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    match parse_generalized_line(&spec, &line) {
                        Ok(_) => {} // Ignore for now
                        Err(e) => errors.push((line_count + 1, e.into_owned())),
                    }
                }
                Ok(None) => break,
                Err(e) => return format!("Error reading line {}! {:?}", line_count + 1, e),
            }
            line_count += 1;
        }
        if !errors.is_empty() {
            return format!(
                "Encountered errors in file \"{}\"!\n{}",
                message.path,
                errors
                    .into_iter()
                    .map(|(line_num, error)| format!("Line {}: {}\n", line_num, error.description))
                    .format("")
            );
        }
    }
    let result = broker.try_send_job(
        StreamJob::new(
            stream! {
                let file = match File::open(format!("gcode/{}", message.path)).await {
                    Ok(file) => file,
                    Err(_e) => {
                        yield GeneralizedLineOwned::Comment("couldn't open file!".to_string());
                        return
                    },
                };
                let file = BufReader::new(file);
                let mut lines = file.lines();
                loop {
                    match lines.next_line().await {
                        Ok(Some(line)) => {
                            match parse_generalized_line(&spec, &line) {
                                Ok(line) => yield line.into_owned(), // Ignore for now
                                Err(_e) => return,
                            }
                        },
                        Ok(None) => return,
                        Err(_e) => return,
                    }
                }
            },
            line_count,
        ),
        MachineHandle {
            write_stream: machine.write_stream.clone(),
            immediate_write_stream: machine.immediate_write_stream.clone(),
        },
    );
    match result {
        Ok(()) => "Job sent!".to_string(),
        Err(_) => "Job not sent!".to_string(),
    }
}

async fn listen_status(ws: WebSocketUpgrade, broker: Extension<Arc<Broker>>) -> Response {
    let mut debug_receiver = broker.watch_status();
    ws.on_upgrade(move |socket| async move {
        let (mut writer, mut reader) = socket.split();
        let (closer, mut close_listen) = oneshot::channel::<()>();
        let (writer, reader) = join! {
            async move {
                // Make sure we send a first message
                let status = debug_receiver.borrow().clone();
                if writer.send(Message::Text(status)).await.is_err() {
                    return writer
                }
                loop {
                    select! {
                        _event = debug_receiver.changed() => {
                            let status = debug_receiver.borrow().clone();
                            if writer.send(Message::Text(status)).await.is_err() {
                                break
                            }
                            sleep(Duration::from_millis(100)).await; //Limit events to once per 100 ms. A little hacky - won't hear close_listen till later.
                        }
                        _ = &mut close_listen => break
                    }
                }
                writer
            },
            async move {
                //ensure that this is actually getting read - so we can handle close frame!
                loop {
                    let response = <SplitStream<WebSocket> as StreamExt>::next(&mut reader).await;
                    if let Some(Ok(Message::Close(_))) = response {
                        closer.send(()).unwrap();
                        break
                    }
                    if response.is_none() {
                        break
                    }
                }
                reader
            }
        };
        let together = reader.reunite(writer).unwrap();
        drop(together.close().await);
    })
}
