use std::pin::Pin;

use futures::{Stream, StreamExt, pin_mut, Future, FutureExt};

use super::{gcode::parser::GeneralizedLineOwned, grbl::standard_handler::{JobHandle, JobFail}};

pub fn sized_stream_to_job<S>(stream: S, total_lines: usize) -> impl FnOnce(JobHandle) -> Pin<Box<dyn Future<Output=()> + Send + 'static>> + Send + 'static
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
                        GeneralizedLineOwned::Line(line) => drop(job_handle.send_gcode(line).await?),
                        GeneralizedLineOwned::Comment(comment) => job_handle.send_comment(comment.to_string()).await?,
                        GeneralizedLineOwned::Empty => {},
                    }
                }
                None => break
            }
        }
        Ok(())
    }.map(|output: Result<(), JobFail>| ()))  // catch and ignore the error!
}