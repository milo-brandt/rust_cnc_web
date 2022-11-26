#![allow(dead_code)]

use std::{sync::Mutex, convert::Infallible, thread};

use axum::{response::{sse::Event, Sse}, extract::{multipart::Field, ContentLengthLimit}, handler::Handler};
use cnc::{grbl::{messages::{GrblStateInfo}, standard_handler::{StandardHandler, ImmediateHandle, MachineDebugEvent, ImmediateMessage, JobHandle}, new_machine::run_machine_with_handler}, stream_job::sized_stream_to_job};
use futures::{Stream, Future, pin_mut};
use hyper::server;
use serde::Serialize;
use tokio::{sync::{mpsc, broadcast, watch}, spawn, time::MissedTickBehavior, io::AsyncWriteExt, fs::{read_dir, remove_file}};
use chrono::offset::Local;
use cnc::machine_writer::BufferCountingWriter;
mod cnc;
mod util;
mod oneway_websocket;
use oneway_websocket::send_stream;
use tokio::runtime::{Runtime, Builder};
use tower_http::catch_panic::CatchPanicLayer;
use util::{history_broadcast, format_bytes::format_byte_string};
use {
    async_stream::stream,
    axum::{
        extract::{
            ws::{Message, WebSocket, WebSocketUpgrade},
            Json, RawBody, Multipart
        },
        response::Response,
        routing::{get, post, delete},
        Extension, Router,
    },
    cnc::{
        gcode::{
            parser::{
                parse_gcode_line, parse_generalized_line, GCodeParseError, GeneralizedLine,
                GeneralizedLineOwned,
            },
            GCodeFormatSpecification,
        },
        grbl::new_machine::{
            ImmediateRequest, WriteRequest,
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

fn make_status_stream(machine: Arc<ImmediateHandle>) -> impl Stream<Item=GrblStateInfo> {
    stream! {
        let mut interval = interval(Duration::from_millis(250));
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            yield machine.get_state().await;
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
async fn status_stream_task(machine: Arc<ImmediateHandle>) -> StatusStreamInfo {
    let (sender, _) = watch::channel(machine.get_state().await);
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
        }
    });
    StatusStreamInfo {
        subscriber_sender
    }
}

fn immediate_command<'a, F, Fut>(action: F) -> impl Handler<(Extension<Arc<ImmediateHandle>>,)>
where
    F: Fn(Arc<ImmediateHandle>) -> Fut + Clone + Send + 'static,
    Fut: Future<Output=()> + Send
{
    move |machine: Extension<Arc<ImmediateHandle>>| {
        async move {
            let fut = action((*machine).clone());
            fut.await;
            "Ok!".to_string()
        }
    }
}




async fn run_server(machine: ImmediateHandle, debug_rx: history_broadcast::Receiver<MachineDebugEvent>) {
    let cors = CorsLayer::new()
    // allow `GET` and `POST` when accessing the resource
    .allow_methods(Any)
    // allow requests from any origin... should maybe read in config
    .allow_origin(Any)
    .allow_headers(Any);
    // We should probably add some other authentication?
    // Maybe a header-to-cookie sort of deal?
    // Or double submit cookie?

    let machine_arc= Arc::new(machine);
    let app = Router::new()
        .route("/job/run_file", post(run_gcode_file))
        .route("/debug/send", post(run_gcode_unchecked))
        .route("/debug/listen_raw", get(listen_raw))
        .route("/debug/listen_status", get(listen_status))
        .route("/debug/listen_position", get(listen_position))
        .route("/job/upload_file", post(upload))
        .route("/job/delete_file", delete(delete_file))
        .route("/list_files", get(get_gcode_list))
        .route("/command/feed_hold", post(immediate_command(|handle| async move { handle.pause().await; })))
        .route("/command/feed_resume", post(immediate_command(|handle| async move { handle.resume().await; })))
        .route("/command/reset", post(immediate_command(|handle| async move { handle.stop().await; })))
        .layer(CatchPanicLayer::new())
        .layer(cors)
        .layer(Extension(machine_arc.clone()))
        .layer(Extension(Arc::new(debug_rx)))
        .layer(Extension(Arc::new(status_stream_task(machine_arc).await)));

    // run it with hyper on localhost:3000
    println!("Listening on port 3000...");
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

fn main() {
    std::panic::set_hook(Box::new(|info| {
        println!("Panicking with {:?}", info);
    }));
    let server_runtime = Builder::new_multi_thread().worker_threads(3).enable_all().build().unwrap();
    let (reader, writer) = server_runtime.block_on(cnc::connection::open_and_reset_arduino_like_serial("/dev/ttyUSB0"));
    let handler_parts = StandardHandler::create(default_settings());
    let handler = handler_parts.handler;
    thread::spawn(move || { // put the machine on a dedicated thread that loves to look at IO
        let machine_runtime = Builder::new_current_thread().enable_all().event_interval(1).build().unwrap();
        let routine = run_machine_with_handler(
            handler,
            BufferCountingWriter::new(writer, 112),
            BufReader::new(reader)
        );
        machine_runtime.block_on(routine);
    });
    server_runtime.block_on(run_server(handler_parts.immediate_handle, handler_parts.debug_rx));
}

/*
#[tokio::main]
async fn main() {
    spawn(machine_future);

    // build our application with a single route

}
*/


//TODO: Put this somewhere it can be serialized and deserialized in common between front and back ends!
async fn listen_position(ws: WebSocketUpgrade, status_stream: Extension<Arc<StatusStreamInfo>>) -> Response {
    let mut receiver = status_stream.subscribe().await;
    send_stream(ws, stream! {
        loop {
            let data = {
                let info = receiver.borrow().clone();
                serde_json::to_string(&info.to_full_info()).unwrap()
            };
            yield Message::Text(data);
            drop(receiver.changed().await);
        }
    })
}

async fn listen_raw(ws: WebSocketUpgrade, debug_receiver: Extension<Arc<history_broadcast::Receiver<MachineDebugEvent>>>) -> Response {
    let mut debug_receiver = debug_receiver.subscribe_with_history_count(100);
    send_stream(ws, stream! { 
        loop {
            let event = debug_receiver.recv().await.unwrap();
            let message = match event {
                MachineDebugEvent::Sent(time, bytes) => format!("> {} {}", time, format_byte_string(bytes)),
                MachineDebugEvent::Received(time, str) => format!("< {} {}", time, str),
                MachineDebugEvent::Warning(time, str) => format!("! {} {}", time, str),
                MachineDebugEvent::Comment(time, str) => format!("~ {} {}", time, str),
            };
            yield Message::Text(message);
        }
    })
}

fn default_settings() -> GCodeFormatSpecification {
    GCodeFormatSpecification {
        axis_letters: b"XYZA".to_vec(),
        offset_axis_letters: b"IJK".to_vec(),
        float_digits: 3,
    }
}
/*async fn run_gcode(
    message: RawBody,
    machine: Extension<Arc<ImmediateHandle>>,
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
}*/
async fn run_gcode_unchecked(
    // Runs the line *if* no job is scheduled yet.
    message: RawBody,
    machine: Extension<Arc<ImmediateHandle>>,
) -> String {
    let mut body_bytes = hyper::body::to_bytes(message.0).await.unwrap().to_vec();
    body_bytes.push(b'\n');
    let result = machine.try_send_job(move |handle: JobHandle| async move {
        unsafe { // Really is unsafe!
            drop(handle.send_gcode_raw(body_bytes).await.unwrap());
        }
    }).await;
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
    machine: Extension<Arc<ImmediateHandle>>,
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
    let result = machine.try_send_job(
        sized_stream_to_job(
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
        )
    ).await;
    match result {
        Ok(()) => "Job sent!".to_string(),
        Err(_) => "Job not sent!".to_string(),
    }
}

async fn listen_status(ws: WebSocketUpgrade, machine: Extension<Arc<ImmediateHandle>>) -> Response {
    let mut debug_receiver = machine.subscribe_job_status().await;
    send_stream(ws, stream! {
        // Make sure we send a first message
        let status = debug_receiver.borrow().clone();
        yield Message::Text(status.unwrap_or_else(|| "Idle".into()));
        loop {
            drop(debug_receiver.changed().await);
            let status = debug_receiver.borrow().clone();
            yield Message::Text(status.unwrap_or_else(|| "Idle".into()));
            sleep(Duration::from_millis(100)).await; //Limit events to once per 100 ms. A little hacky - won't hear close_listen till later.
        }
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
#[derive(Deserialize)]
struct DeleteGcodeFile {
    path: String,
}

async fn delete_file(info: Json<DeleteGcodeFile>) -> String {
    //TODO: Scope where we can delete :)
    remove_file(format!("gcode/{}", info.path)).await.unwrap();
    "Ok".to_string()
}

async fn get_gcode_list() -> Json<Vec<String>> {
    let mut entries = read_dir("gcode").await.unwrap();
    let mut values = Vec::new();
    while let Some(entry) = entries.next_entry().await.unwrap() {
        values.push(entry.file_name().into_string().unwrap());
    }
    Json(values)
}