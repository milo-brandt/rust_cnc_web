use async_stream::try_stream;
use clap::Parser;
use common::api;
use futures::{
    channel::mpsc,
    SinkExt, StreamExt, Stream, TryStream, TryStreamExt, future::ready,
};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher, Config, EventKind, event::{AccessKind, AccessMode, ModifyKind, RenameMode, RemoveKind, CreateKind}};
use tokio::{fs::{canonicalize, DirEntry, read_dir}, time::sleep};
use std::{path::{Path, PathBuf, Component}, fs::{File, self}, io::Read, str::FromStr, time::Duration, task::ready, pin::pin};

#[derive(clap::Parser, Debug)]
#[command(author = "Milo Brandt", version = "0.1.0", about = "Sync local files to the cnc server.", long_about = None)]
struct Args {
    /// Name of the person to greet
    #[arg()]
    local_directory: String,
    #[arg()]
    remote_directory: String,
}

// All paths should be relative to the watch.
enum Change {
    Added(PathBuf),
    Deleted(PathBuf),
}

async fn upload_file(client: &mut reqwest::Client, local_path: PathBuf, remote_path: PathBuf) {
    let form = reqwest::multipart::Form::new()
        .text("filename", remote_path.to_string_lossy().to_string())
        .part("file", reqwest::multipart::Part::bytes(tokio::fs::read(local_path.clone()).await.unwrap()).file_name("file.nc"));
    let response = client.post(format!("http://cnc:3000{}", api::UPLOAD_GCODE_FILE))
        .multipart(form)
        .send()
        .await.unwrap();
    println!("Uploaded ({}): {} > {}", response.status(), local_path.to_string_lossy(), remote_path.to_string_lossy())
}
async fn delete_file(client: &mut reqwest::Client, remote_path: PathBuf) {
    let data = api::DeleteGcodeFile {
        path: remote_path.to_string_lossy().to_string(),
        is_directory: false,
    };
    let response = client.delete(format!("http://cnc:3000{}", api::DELETE_GCODE_FILE))
        .json(&data)
        .send()
        .await.unwrap();
    println!("Deleted ({}): {}", response.status(), remote_path.to_string_lossy())
}

fn is_path_actionable(path: &Path) -> bool {
    // Only upload files that are .nc files and don't have any part starting with a dot.
    path.extension().and_then(|ext| ext.to_str()) == Some("nc")
    && path.components().all(|component| {
        if let Component::Normal(part) = component {
            part.to_str().map_or(false, |part| !part.starts_with("."))
        } else {
            true
        }
    })
}

pub fn files_in_directory_recursive(path: PathBuf) -> impl Stream<Item=tokio::io::Result<PathBuf>> {
    try_stream! {
        let root = path;
        let mut paths_to_read = vec![root.clone()];
        while let Some(path) = paths_to_read.pop() {
            let mut dir_reader = read_dir(path).await?;
            while let Some(dir_entry) = dir_reader.next_entry().await? {
                let file_type = dir_entry.file_type().await?;
                if file_type.is_dir() {
                    paths_to_read.push(dir_entry.path().to_path_buf());
                } else if file_type.is_file() {
                    yield dir_entry.path().strip_prefix(&root).unwrap().to_path_buf();
                }
            }
        }
    }
}

/// Async, futures channel based event watching
#[tokio::main]
async fn main() {
    let args = Args::parse();
    let local_directory = canonicalize(args.local_directory).await.unwrap();

    let (events_tx, mut events_rx) = mpsc::unbounded();
    tokio::spawn(async_watch(local_directory.clone(), events_tx));
    let mut client = reqwest::Client::new();
    // First: upload anything interesting already in the directory.
    let mut existing_paths = pin!(files_in_directory_recursive(local_directory.clone())
        .try_filter(|path| ready(is_path_actionable(&path))));

    // Ideally: could use try_for_each, but couldn't borrow from the owning scope there. Perhaps a phantom lifetime is needed on that function? Or maybe
    // it's just not possible.
    while let Some(item) = existing_paths.next().await {
        let path = item.unwrap();
        println!("Uploading existing file {}", path.to_string_lossy());
        let local_path = local_directory.clone().join(&path);
        let remote_path = PathBuf::from(args.remote_directory.clone()).join(&path);
        upload_file(&mut client, local_path, remote_path).await;
    }
    loop {
        let next = events_rx.next().await.unwrap();
        sleep(Duration::from_millis(500)).await; // Throttle changes, give time for things to settle out if needed.
        match next {
            Change::Added(path) => {
                if !is_path_actionable(&path) { println!("Irrelevant {:?}", path); continue; }
                println!("Uploading {}", path.to_string_lossy());
                let local_path = local_directory.clone().join(&path);
                let remote_path = PathBuf::from(args.remote_directory.clone()).join(&path);
                upload_file(&mut client, local_path, remote_path).await;
            },
            Change::Deleted(path) => {
                if !is_path_actionable(&path) { println!("Irrelevant {:?}", path); continue; }
                println!("Deleting {}", path.to_string_lossy());
                let remote_path = PathBuf::from(args.remote_directory.clone()).join(&path);
                delete_file(&mut client, remote_path).await;
            }
        }
    }
}

fn async_watcher() -> notify::Result<(RecommendedWatcher, mpsc::UnboundedReceiver<notify::Result<Event>>)> {
    let (tx, rx) = mpsc::unbounded();

    // Automatically select the best implementation for your platform.
    // You can also access each implementation directly e.g. INotifyWatcher.
    let watcher = RecommendedWatcher::new(move |res| {
        drop(tx.unbounded_send(res));
    }, Config::default())?;

    Ok((watcher, rx))
}

async fn async_watch<P: AsRef<Path>>(path: P, mut output: mpsc::UnboundedSender<Change>) -> notify::Result<()> {
    let (mut watcher, mut rx) = async_watcher()?;

    // Add a path to be watched. All files and directories at that path and
    // below will be monitored for changes.
    watcher.watch(path.as_ref(), RecursiveMode::Recursive)?;

    let path = path.as_ref().to_path_buf();

    let get_relative_path = |input: &Path| {
        input.strip_prefix(&path).unwrap().to_path_buf()
    };

    while let Some(res) = rx.next().await {
        match res {
            Ok(event) => {
                println!("EVENT: {:?}", event);
                match event.kind {
                    EventKind::Access(AccessKind::Close(AccessMode::Write)) => {
                        output.send(Change::Added(get_relative_path(&event.paths[0]))).await.unwrap();
                    },
                    EventKind::Create(CreateKind::File) => {
                        output.send(Change::Added(get_relative_path(&event.paths[0]))).await.unwrap();
                    },
                    EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
                        output.send(Change::Deleted(get_relative_path(&event.paths[0]))).await.unwrap();
                    },
                    EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
                        output.send(Change::Added(get_relative_path(&event.paths[0]))).await.unwrap();
                    },
                    /*EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
                        output.send(Change::Added(get_relative_path(&event.paths[1]))).await.unwrap();
                        output.send(Change::Deleted(get_relative_path(&event.paths[0]))).await.unwrap();
                    },*/ // Seems like the From and To sides are both sent in this case?
                    EventKind::Remove(_) => {
                        output.send(Change::Deleted(get_relative_path(&event.paths[0]))).await.unwrap();
                    }
                    _ => {}
                }
            },
            Err(e) => println!("watch error: {:?}", e),
        }
    }

    Ok(())
}