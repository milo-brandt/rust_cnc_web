use std::pin::Pin;

use futures::{Stream, StreamExt, pin_mut, Future, FutureExt, SinkExt};
use tokio::sync::mpsc;

use crate::cnc::gcode::{GCodeLine, GCodeCommand};

use super::{gcode::parser::GeneralizedLineOwned, grbl::{standard_handler::{JobHandle, JobFail}, messages::ProbeEvent}};

pub fn sized_stream_to_job<S>(stream: S, total_lines: usize, results: mpsc::Sender<ProbeEvent>) -> impl FnOnce(JobHandle) -> Pin<Box<dyn Future<Output=()> + Send + 'static>> + Send + 'static
where
    S: Stream<Item=GeneralizedLineOwned> + Send + 'static
{
    move |job_handle| Box::pin(async move {
        job_handle.set_status("Starting job...".into()).await?;
        let mut line_num = 0;
        pin_mut!(stream);
        loop {
            match stream.next().await {
                Some(v) => {
                    line_num += 1;
                    job_handle.set_status(format!("At line {}/{}", line_num, total_lines)).await?;
                    match v {
                        // If we wished, the next line could have its future handled for whether we get "ok" or "error".
                        GeneralizedLineOwned::Line(line) => {
                            if line.command.as_ref().is_some_and(|v| if let GCodeCommand::Probe { .. } = &v { true } else { false }) {
                                let (line_result, probe_result) = job_handle.send_probe_gcode(line).await?;
                                line_result.await.map_err(|_| JobFail)?;
                                let probe_event = probe_result.await.map_err(|_| JobFail)?;
                                job_handle.send_comment(format!(
                                    "PROBE RESULT: {}",
                                    serde_json::to_string(&probe_event).unwrap()
                                )).await?;
                                drop(results.send(probe_event).await);
                            } else {
                                drop(job_handle.send_gcode(line).await?)
                            }
                        },
                        GeneralizedLineOwned::Comment(comment) => job_handle.send_comment(comment.to_string()).await?,
                        GeneralizedLineOwned::Empty => {},
                    }
                }
                None => {
                    // Send a dwell to synchronize at end.
                    job_handle.set_status(format!("All lines sent. Waiting for machine to finish.")).await?;
                    drop(job_handle.send_gcode(GCodeLine {
                        modals: Vec::new(),
                        command: Some(GCodeCommand::Dwell { duration: 0.01 }),
                    }).await?.await);
                    return Ok(())
                }
            }
        }
    }.map(|_: Result<(), JobFail>| ()))  // catch and ignore the error!
}