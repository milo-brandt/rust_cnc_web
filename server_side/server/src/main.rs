#![allow(dead_code)]

use std::{sync::Mutex, convert::Infallible, thread, collections::HashMap, borrow::Borrow, path::{PathBuf, Path}, fs::FileType, env};

use axum::{response::{sse::Event, Sse}, extract::{multipart::Field, self, DefaultBodyLimit}, handler::Handler, body::{StreamBody, BoxBody}, routing::MethodRouter};
use cnc::{grbl::{messages::{GrblStateInfo}, standard_handler::{StandardHandler, ImmediateHandle, MachineDebugEvent, ImmediateMessage, JobHandle}, new_machine::run_machine_with_handler}, stream_job::sized_stream_to_job, gcode::{geometry::{as_lines_simple, as_lines_from_best_start}, AxisValues}};
use futures::{Stream, Future, pin_mut};
use hyper::{server, Body};
use paths::lexically_normal_path;
use serde::Serialize;
use tempdir::TempDir;
use tokio::{sync::{mpsc, broadcast, watch}, spawn, time::MissedTickBehavior, io::AsyncWriteExt, fs::{read_dir, remove_file, create_dir_all, remove_dir_all, rename}};
use chrono::{offset::Local, Utc};
use cnc::machine_writer::BufferCountingWriter;
mod cnc;
mod paths;
mod server_result;
mod util;
mod oneway_websocket;
mod coordinates;
use oneway_websocket::send_stream;
use tokio::runtime::{Runtime, Builder};
use tokio_util::io::{StreamReader, ReaderStream};
use tower_http::{catch_panic::CatchPanicLayer, trace::TraceLayer};
use util::{history_broadcast, format_bytes::format_byte_string, force_output_type};
use common::api;
use clap::Parser;
use anyhow::{anyhow, Context};
use server_result::{ServerResult, ServerError};

#[derive(Parser, Debug)]
#[command(author = "Milo Brandt", version = "0.1.0", about = "Run a server connected to the given port.", long_about = None)]
struct Args {
    /// Name of the person to greet
    #[arg(short, long)]
    port: String,
    #[arg(short, long)]
    data_folder: String,
}

pub struct Config {
    data_folder: PathBuf
}

impl Config {
    pub fn gcode_root(&self) -> PathBuf {
        self.data_folder.join("gcode")
    }
    pub fn gcode_path(&self, path: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
        match lexically_normal_path(path.as_ref()) {
            None => Err(anyhow!("Invalid path! {:?}", path.as_ref())),
            Some(path) => {
                let mut result = self.gcode_root();
                result.push(path);
                Ok(result)
            }
        }
    }
    pub fn jobs_root(&self) -> PathBuf {
        self.data_folder.join("jobs")
    }
    pub fn new_job_path(&self) -> PathBuf {
        self.data_folder.join("jobs").join(format!("job_{}", Local::now().timestamp()))
    }
    pub fn job_path(&self, path: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
        match lexically_normal_path(path.as_ref()) {
            None => Err(anyhow!("Invalid path! {:?}", path.as_ref())),
            Some(path) => {
                let mut result = self.jobs_root();
                result.push(path);
                Ok(result)
            }
        }
    }
}


use crate::cnc::grbl::handler::SpeedOverride;
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

fn immediate_command<'a, F, Fut>(action: F) -> MethodRouter<(), Body, Infallible>
where
    F: Fn(Arc<ImmediateHandle>) -> Fut + Clone + Send + 'static,
    Fut: Future<Output=()> + Send
{
    post(move |machine: Extension<Arc<ImmediateHandle>>| {
        async move {
            let fut = action((*machine).clone());
            fut.await;
            "Ok!".to_string()
        }
    })
}

struct CoordinateOffsets {
    offsets: Mutex<HashMap<String, Vec<f64>>>
}
impl CoordinateOffsets {
    pub fn new() -> Self {
        CoordinateOffsets { offsets: Mutex::new(HashMap::new()) }
    }
}




async fn run_server(machine: ImmediateHandle, debug_rx: history_broadcast::Receiver<MachineDebugEvent>, config: Config) {
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
        .route(api::RUN_GCODE_FILE, post(run_gcode_file))
        .route(api::UPLOAD_GCODE_FILE, post(upload))
        .route(api::CREATE_GCODE_DIRECTORY, post(create_directory))
        .route(api::DELETE_GCODE_FILE, delete(delete_file))
        .route(api::LIST_GCODE_FILES, post(get_gcode_list))
        .route(api::EXAMINE_LINES_IN_GCODE_FILE, post(get_gcode_file_positions))
        .route(&format!("{}/*path", api::DOWNLOAD_GCODE), get(download_gcode_file))
        .route("/job/list/*path", get(get_gcode_list_better))
        .route("/job/list/", get(get_gcode_list_better))
        .route("/job/examine/*path", get(get_gcode_file_positions_better))

        .route(&format!("{}/*path", api::DOWNLOAD_RESULTS), get(download_job_results))
        .route(api::LIST_RESULTS, get(get_results_list))

        .route(api::SEND_RAW_GCODE, post(run_gcode_unchecked))
        .route(api::LISTEN_TO_RAW_MACHINE, get(listen_raw))
        .route(api::LISTEN_TO_JOB_STATUS, get(listen_status))
        .route(api::LISTEN_TO_MACHINE_STATUS, get(listen_position))
        
        .route(api::COMMAND_PAUSE, (immediate_command(|handle| async move { handle.pause().await; })))
        .route(api::COMMAND_RESUME, (immediate_command(|handle| async move { handle.resume().await; })))
        .route(api::COMMAND_STOP, (immediate_command(|handle| async move { handle.stop().await; })))
        .route(api::COMMAND_RESET, (immediate_command(|handle| async move { handle.reset().await; })))

        .route(api::FEED_OVERRIDE.reset, (immediate_command(|handle| async move { handle.override_speed(SpeedOverride::FeedReset).await; })))
        .route(api::FEED_OVERRIDE.plus_10, (immediate_command(|handle| async move { handle.override_speed(SpeedOverride::FeedIncrease10).await; })))
        .route(api::FEED_OVERRIDE.plus_1, (immediate_command(|handle| async move { handle.override_speed(SpeedOverride::FeedIncrease1).await; })))
        .route(api::FEED_OVERRIDE.minus_1, (immediate_command(|handle| async move { handle.override_speed(SpeedOverride::FeedDecrease1).await; })))
        .route(api::FEED_OVERRIDE.minus_10, (immediate_command(|handle| async move { handle.override_speed(SpeedOverride::FeedDecrease10).await; })))

        .route(api::SPINDLE_OVERRIDE.reset, (immediate_command(|handle| async move { handle.override_speed(SpeedOverride::SpindleReset).await; })))
        .route(api::SPINDLE_OVERRIDE.plus_10, (immediate_command(|handle| async move { handle.override_speed(SpeedOverride::SpindleIncrease10).await; })))
        .route(api::SPINDLE_OVERRIDE.plus_1, (immediate_command(|handle| async move { handle.override_speed(SpeedOverride::SpindleIncrease1).await; })))
        .route(api::SPINDLE_OVERRIDE.minus_1, (immediate_command(|handle| async move { handle.override_speed(SpeedOverride::SpindleDecrease1).await; })))
        .route(api::SPINDLE_OVERRIDE.minus_10, (immediate_command(|handle| async move { handle.override_speed(SpeedOverride::SpindleDecrease10).await; })))
     
        .route(api::RAPID_OVERRIDE.reset, (immediate_command(|handle| async move { handle.override_speed(SpeedOverride::RapidReset).await; })))
        .route(api::RAPID_OVERRIDE.half, (immediate_command(|handle| async move { handle.override_speed(SpeedOverride::RapidHalf).await; })))
        .route(api::RAPID_OVERRIDE.quarter, (immediate_command(|handle| async move { handle.override_speed(SpeedOverride::RapidQuarter).await; })))

        .route("/command/home", (immediate_command(|handle| async move { handle.override_speed(SpeedOverride::RapidQuarter).await; })))

        .route(api::SHUTDOWN, post(shutdown))

        .nest("/coords", coordinates::get_service(&config).await.unwrap())

        .layer(TraceLayer::new_for_http())
        .layer(CatchPanicLayer::new())
        .layer(cors)
        .layer(DefaultBodyLimit::max(10_000_000))
        .layer(Extension(machine_arc.clone()))
        .layer(Extension(Arc::new(debug_rx)))
        .layer(Extension(Arc::new(status_stream_task(machine_arc).await)))
        .layer(Extension(Arc::new(CoordinateOffsets::new())))
        .layer(Extension(Arc::new(config)));

    // run it with hyper on localhost:3000
    println!("Listening on port 3000...");
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}



fn main() {
    let args = Args::parse();
    println!("Starting CNC server with configuration: {:?}", args);
    std::panic::set_hook(Box::new(|info| {
        println!("Panicking with {:?}", info);
    }));
    tracing_subscriber::fmt::init();
    let server_runtime = Builder::new_multi_thread().worker_threads(3).enable_all().build().unwrap();
    println!("Opening port...");
    let (reader, writer) = server_runtime.block_on(cnc::connection::open_and_reset_arduino_like_serial(&args.port));
    let handler_parts = StandardHandler::create(default_settings());
    let handler = handler_parts.handler;
    println!("Starting threads for machine and web communication...");
    thread::spawn(move || { // put the machine on a dedicated thread that loves to look at IO
        let machine_runtime = Builder::new_current_thread().enable_all().event_interval(1).build().unwrap();
        let routine = run_machine_with_handler(
            handler,
            BufferCountingWriter::new(writer, 112),
            BufReader::new(reader)
        );
        machine_runtime.block_on(routine);
    });
    server_runtime.block_on(run_server(
        handler_parts.immediate_handle,
        handler_parts.debug_rx,
        Config{ data_folder: PathBuf::from(args.data_folder) }
    )
    );
}

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
async fn run_gcode_unchecked(
    // Runs the line *if* no job is scheduled yet.
    machine: Extension<Arc<ImmediateHandle>>,
    message: RawBody,
) -> ServerResult<String> {
    let mut body_bytes = hyper::body::to_bytes(message.0).await.unwrap().to_vec();
    body_bytes.push(b'\n');
    let result = machine.try_send_job(move |handle: JobHandle| async move {
        unsafe { // Really is unsafe!
            drop(handle.send_gcode_raw(body_bytes).await.unwrap());
        }
    }).await;
    match result {
        Ok(()) => Ok("Job sent!".to_string()),
        Err(_) => Err(anyhow!("Job not sent!").into()),
    }
}


fn axis_value_to_array(v: &AxisValues) -> [f32; 3] {
    let mut result = [0.0, 0.0, 0.0];
    for (coord, value) in &v.0 {
        if coord < &3 {
            result[*coord] = *value as f32;
        }
    }
    result
}

async fn download_gcode_file(
    path: extract::Path<String>,
    config: Extension<Arc<Config>>,
) -> ServerResult<Response> {
    let path = config.gcode_path(&*path)?;
    let file = File::open(path).await?;
    let body = BoxBody::new(StreamBody::new(ReaderStream::new(file)));
    /* */
    let response = Response::builder()
        .header("Content-Type", "text/plain;")
        .header("Content-Disposition", "inline;")
        .body(body)?;
    Ok(response)
}

async fn get_gcode_file_positions(
    config: Extension<Arc<Config>>,
    message: Json<api::ExamineGcodeFile>,
) -> ServerResult<Json<Vec<[f32; 3]>>> {
    let mut program = Vec::new();

    /*
        A lot of code duplication; should perhaps make an iterator that just reads through a 
        GCode file and parses it... (or Stream I guess?)
    */
    let file = match File::open(config.gcode_path(&message.path)?).await {
        Ok(file) => file,
        Err(e) => return Err(anyhow!("Error! {:?}", e).into()),
    };
    let file = BufReader::new(file);
    let mut lines = file.lines();
    let spec = default_settings();
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => {
                match parse_generalized_line(&spec, &line) {
                    Ok(GeneralizedLine::Line(line)) => {
                        program.push(line);
                    }
                    Ok(_) => {}
                    Err(e) => return Err(anyhow!("Error! {:?}", e).into()),
                }
            }
            Ok(None) => break,
            Err(e) => return Err(anyhow!("Error: {:?}", e).into()),
        }
    }
    let lines = as_lines_from_best_start(&program);
    let lines = match lines {
        Err(e) => return Err(anyhow!("Error! {:?}", e).into()),
        Ok(lines) => lines,
    };
    Ok(Json(lines.iter().map(axis_value_to_array).collect()))
} 

async fn get_gcode_file_positions_better(
    config: Extension<Arc<Config>>,
    path: extract::Path<String>,
) -> ServerResult<Json<Vec<[f32; 3]>>> {
    let mut program = Vec::new();
    let path = config.gcode_path(&*path)?;
    /*
        A lot of code duplication; should perhaps make an iterator that just reads through a 
        GCode file and parses it... (or Stream I guess?)
    */
    let file = match File::open(path).await {
        Ok(file) => file,
        Err(e) => return Err(anyhow!("Error! {:?}", e).into()),
    };
    let file = BufReader::new(file);
    let mut lines = file.lines();
    let spec = default_settings();
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => {
                match parse_generalized_line(&spec, &line) {
                    Ok(GeneralizedLine::Line(line)) => {
                        program.push(line);
                    }
                    Ok(_) => {}
                    Err(e) => return Err(anyhow!("Error! {:?}", e).into()),
                }
            }
            Ok(None) => break,
            Err(e) => return Err(anyhow!("Error: {:?}", e).into()),
        }
    }
    let lines = as_lines_from_best_start(&program);
    let lines = match lines {
        Err(e) => return Err(anyhow!("Error! {:?}", e).into()),
        Ok(lines) => lines,
    };
    Ok(Json(lines.iter().map(axis_value_to_array).collect()))
} 


async fn run_gcode_file(
    machine: Extension<Arc<ImmediateHandle>>,
    config: Extension<Arc<Config>>,
    message: Json<api::RunGcodeFile>,
) -> ServerResult<String> {
    let path = config.gcode_path(&message.path)?;
    let spec = default_settings();
    let mut line_count = 0;
    {
        let file = match File::open(&path).await {
            Ok(file) => file,
            Err(e) => return Err(anyhow!("Error! {:?}", e).into()),
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
                Err(e) => return Err(anyhow!("Error reading line {}! {:?}", line_count + 1, e).into()),
            }
            line_count += 1;
        }
        if !errors.is_empty() {
            return Err(anyhow!(
                "Encountered errors in file \"{}\"!\n{}",
                message.path,
                errors
                    .into_iter()
                    .map(|(line_num, error)| format!("Line {}: {}\n", line_num, error.description))
                    .format("")
            ).into());
        }
    }
    let (results_tx, mut results_rx) = mpsc::channel(128);
    let result = machine.try_send_job(
        sized_stream_to_job(
            stream! {
                let file = match File::open(&path).await {
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
            results_tx,
        )
    ).await;
    let dirname = config.new_job_path();
    spawn(force_output_type::<anyhow::Result<()>>(async move {
        let mut result = Vec::new();
        while let Some(v) = results_rx.recv().await {
            result.push(v);
        }
        if !result.is_empty() {
            let filename = dirname.join("probes.json");
            create_dir_all(dirname).await?;
            let mut file = File::create(filename).await?;
            file.write_all(serde_json::to_string(&result)?.as_bytes()).await?;
        }
        Ok(())
    }));
    match result {
        Ok(()) => Ok("Job sent!".to_string()),
        Err(_) => Err(anyhow!("Job not sent!").into()),
    }
}

async fn listen_status(ws: WebSocketUpgrade, machine: Extension<Arc<ImmediateHandle>>) -> Response {
    let mut debug_receiver = machine.subscribe_job_status().await;
    send_stream(ws, stream! {
        // Make sure we send a first message
        loop {
            let status = debug_receiver.borrow().clone();
            yield Message::Text(serde_json::to_string(&status).unwrap());
            let sleeper = sleep(Duration::from_millis(100));
            drop(debug_receiver.changed().await);
            sleeper.await; //Limit events to once per 100 ms.
        }
    })
}


async fn dump_field_to_file(mut file: File, mut field: Field<'_>) -> anyhow::Result<()> {
    while let Some(bytes) = field.chunk().await? {
        file.write_all(&bytes).await?
    }
    Ok(())
}

// Limits file size to 128 MiB.
async fn upload(config: Extension<Arc<Config>>, mut multipart: Multipart) -> ServerResult<String> {
    let mut file_name = None::<PathBuf>;
    let tmp_dir = TempDir::new("file_download")?;
    let tmp_path = tmp_dir.path().join("file");
    let mut has_file = false;
    while let Some(field) = multipart.next_field().await.unwrap() {
        let name = field.name().unwrap().to_string();
        if name == "file" {
            if(has_file) {
                return Err(anyhow!("Multiple files given!").into());
            }
            has_file = true;
            dump_field_to_file(
                File::create(tmp_path.clone()).await?,
                field
            ).await?;
        } else if name == "filename" {
            let presumptive_name = field.text().await?;
            file_name = Some(
                config.gcode_path(&presumptive_name)?
            );
        }
    }
    if !has_file {
        return Err(anyhow!("No file given!").into());
    }
    let file_name = file_name.ok_or_else(|| anyhow!("No filename given!"))?;
    // This is not quite right... but it's probably fine
    let directory = file_name.parent().ok_or(anyhow!("Cannot specify top level as filename!"))?;
    create_dir_all(directory).await?;
    rename(tmp_path, file_name).await?;
    Ok("Uploaded!".into())
}
async fn create_directory(config: Extension<Arc<Config>>, info: Json<api::CreateGcodeDirectory>) -> ServerResult<String> {
    create_dir_all(config.gcode_path(&info.directory)?).await?;
    Ok("Ok".to_string())
}
async fn delete_file(config: Extension<Arc<Config>>, info: Json<api::DeleteGcodeFile>) -> ServerResult<String> {
    let old_path = config.gcode_path(&info.path)?;
    if old_path == PathBuf::new() {
        return Err(ServerError::bad_request("cannot delete root directory".to_string()));
    }
    let new_folder = config.data_folder.join("deleted_gcode").join(Utc::now().to_string());
    create_dir_all(&new_folder).await?;
    let new_path = new_folder.join(old_path.file_name().unwrap_or(std::ffi::OsStr::new("Unknown")));
    println!("MOVING {:?} to {:?}", old_path, new_path);
    rename(old_path, new_path).await?;
    /*
    if(info.is_directory) {
        remove_dir_all(config.gcode_path(&info.path)?).await?;
    } else {
        remove_file(config.gcode_path(&info.path)?).await?;
    }
    */
    Ok("Ok".to_string())
}

async fn get_gcode_list(config: Extension<Arc<Config>>, info: Json<api::ListGcodeFiles>) -> ServerResult<Json<Vec<api::GcodeFile>>> {
    let mut entries = read_dir(config.gcode_path(&info.prefix)?).await?;
    let mut values = Vec::new();
    while let Some(entry) = entries.next_entry().await.unwrap() {
        values.push(api::GcodeFile {
            name: entry.file_name().into_string().unwrap(),
            is_file: entry.file_type().await?.is_file(),
        });
    }
    values.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Json(values))
}
async fn get_gcode_list_better(path: Option<extract::Path<String>>, config: Extension<Arc<Config>>) -> ServerResult<Json<Vec<api::GcodeFile>>> {
    let mut entries = read_dir(config.gcode_path(path.as_ref().map_or("", |path| path.as_str()))?).await?;
    let mut values = Vec::new();
    while let Some(entry) = entries.next_entry().await.unwrap() {
        values.push(api::GcodeFile {
            name: entry.file_name().into_string().unwrap(),
            is_file: entry.file_type().await?.is_file(),
        });
    }
    values.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Json(values))
}

async fn shutdown() -> String {
    if env::var("NO_SHUTDOWN") == Ok("1".to_string()) {
        format!("Shutdown disabled!")
    } else {
        match system_shutdown::shutdown() {
            Ok(()) => "Bye!".to_string(),
            Err(err) => format!("Failed: {:?}", err),
        }
    }
}

async fn download_job_results(
    path: extract::Path<String>,
    config: Extension<Arc<Config>>,
) -> ServerResult<Response> {
    let path = config.job_path(&*path)?.join("probes.json");
    let file = File::open(path).await?;
    let body = BoxBody::new(StreamBody::new(ReaderStream::new(file)));
    /* */
    let response = Response::builder()
        .header("Content-Type", "text/plain;")
        .header("Content-Disposition", "inline;")
        .body(body)?;
    Ok(response)
}

async fn get_results_list(config: Extension<Arc<Config>>) -> ServerResult<Json<Vec<String>>> {
    let mut entries = read_dir(config.jobs_root()).await?;
    let mut values = Vec::new();
    while let Some(entry) = entries.next_entry().await.unwrap() {
        values.push(entry.file_name().into_string().map_err(|_| anyhow!("Failed to unwrap file name"))?);
    }
    values.sort_by(|a, b| b.cmp(&a));
    Ok(Json(values))
}
