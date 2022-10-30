#![allow(dead_code)]

use std::{sync::Mutex, convert::Infallible};

use axum::{response::{sse::Event, Sse}, extract::{multipart::Field, ContentLengthLimit}};
use cnc::grbl::messages::{GrblStateInfo};
use futures::{Stream, Future, pin_mut};
use tokio::{sync::{mpsc, broadcast, watch}, spawn, time::MissedTickBehavior, io::AsyncWriteExt};

mod cnc;
mod util;

use {
    async_stream::stream,
    axum::{
        extract::{
            ws::{Message, WebSocket, WebSocketUpgrade},
            Json, RawBody, Multipart
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
        fs::{File, OpenOptions},
        io::{AsyncBufReadExt, BufReader},
        join, select,
        sync::oneshot,
        time::{sleep, interval},
    },
    tower_http::cors::{Any, CorsLayer},
};

fn make_status_stream(machine: Arc<MachineInterface>) -> impl Stream<Item=GrblStateInfo> {
    stream! {
        let mut interval = interval(Duration::from_millis(250));
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            let (sender, receiver) = oneshot::channel();
            machine.immediate_write_stream.send(ImmediateRequest::Status { result: sender }).await.unwrap();
            yield receiver.await.unwrap();
        }
    }
}

struct StatusStreamInfo {
    subscriber_sender: mpsc::Sender<oneshot::Sender<watch::Receiver<GrblStateInfo>>>
}
impl StatusStreamInfo {
    async fn subscribe(&self) -> watch::Receiver<GrblStateInfo> {
        let (sender, receiver) = oneshot::channel();
        self.subscriber_sender.send(sender).await.unwrap();
        receiver.await.unwrap()
    }
}
async fn status_stream_task(machine: Arc<MachineInterface>) -> StatusStreamInfo {
    let (sender, receiver) = oneshot::channel();
    machine.immediate_write_stream.send(ImmediateRequest::Status { result: sender }).await.unwrap();
    let (sender, _) = watch::channel(receiver.await.unwrap());
    let (subscriber_sender, mut subscriber_receiver) = mpsc::channel::<oneshot::Sender<watch::Receiver<GrblStateInfo>>>(16);
    spawn(async move {
        let mut subscribers_closed = false;
        while !subscribers_closed {
            match subscriber_receiver.recv().await {
                Some(subscriber) => {
                    drop(subscriber.send(sender.subscribe()));
                }
                None => return
            }
            println!("Starting stream!");
            let stream = make_status_stream(machine.clone());
            pin_mut!(stream);
            let mut next_stream = stream.next();
            loop {
                select! {
                    new_subscriber = subscriber_receiver.recv(), if !subscribers_closed => {
                        match new_subscriber {
                            Some(new_subscriber) => drop(new_subscriber.send(sender.subscribe())),
                            None => {
                                subscribers_closed = true;
                            }
                        }
                    },
                    new_value = &mut next_stream => {
                        next_stream = stream.next();
                        match new_value {
                            Some(new_value) => match sender.send(new_value) {
                                Ok(()) => {}
                                Err(_) => break // No listeners - stop computation...
                            },
                            None => return // Stream is done???
                        }
                    }
                }
            }
            println!("No more subscribers!")
        }
    });
    StatusStreamInfo {
        subscriber_sender
    }
}



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
    let machine_arc = Arc::new(machine);
    
    // build our application with a single route
    let app = Router::new()
        .route("/job/run_file", post(run_gcode_file))
        .route("/debug/send", post(index))
        .route("/debug/gcode_job", post(run_gcode))
        .route("/debug/gcode_unchecked_if_free", post(run_gcode_unchecked))
        .route("/debug/listen_raw", get(listen_raw))
        .route("/debug/listen_status", get(listen_status))
        .route("/debug/listen_machine_status", get(listen_machine_status))
        .route("/upload", post(upload))
        //.route("/ws", get(websocket_upgrade))
        .layer(cors)
        .layer(Extension(machine_arc.clone()))
        .layer(Extension(Arc::new(Broker::new())))
        .layer(Extension(Arc::new(status_stream_task(machine_arc).await)));

    // run it with hyper on localhost:3000
    println!("Listening on port 3000...");
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn listen_machine_status(status_stream: Extension<Arc<StatusStreamInfo>>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    println!("Request to listen to status!");
    let mut receiver = status_stream.subscribe().await;
    let result = stream! {
        loop {
            let data = format!("[{}]", receiver.borrow().machine_position.indexed_iter().map(|(_, v)| v).format(", "));
            yield Event::default().data(data);
            if let Err(_) = receiver.changed().await {
                break
            }
        }
    }.map(Ok);
    Sse::new(result)
}

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
        || c == '_' || c == '.'
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


async fn dump_field_to_file(mut file: File, mut field: Field<'_>) {
    while let Some(bytes) = field.chunk().await.unwrap() {
        file.write_all(&bytes).await.unwrap();
    }
}

// Limits file size to 128 MiB.
async fn upload(multipart: ContentLengthLimit<Multipart, 134217728>) -> String {
    let mut multipart = multipart.0;
    let mut file_name = None;
    while let Some(field) = multipart.next_field().await.unwrap() {
        let name = field.name().unwrap().to_string();
        if name == "file" {
            match file_name.take() {
                None => return "Filename not given before file!".to_string(),
                Some(file_name) => {
                    dump_field_to_file(
                        File::create(format!("gcode/{}", file_name)).await.unwrap(),
                        field
                    ).await
                }
            }
            return "Uploaded!".to_string();
        } else if name == "filename" {
            let presumptive_name = field.text().await.unwrap();
            if !presumptive_name.chars().all(|c|
                c.is_ascii_alphanumeric()
                || c == '_' || c == '.'
            ) || presumptive_name.len() > 255 {
                return "Illegal filename!".to_string();
            }
            file_name = Some(presumptive_name);
        }
    }
    "File not given!".to_string()
}