use tokio::{process::Command, sync::oneshot, select, fs::create_dir_all};


#[tokio::main]
async fn main() {
    let (sender, receiver) = oneshot::channel();
    let mut sender = Some(sender);
    ctrlc::set_handler(move || {
        if let Some(sender) = sender.take() {
            drop(sender.send(()));
            println!("Shutting down!");
        } else {
            panic!("Shutting down without grace!");
        }
    }).unwrap();
    // Create the root to store data, if not already there.
    create_dir_all("../test_data").await.unwrap();
    create_dir_all("../test_data/gcode").await.unwrap();

    let command_port = machine_mock::socat_port::port_to_command(|input, output| {
        machine_mock::slow::trivial_machine(input, output)
    }).await.unwrap();
    let mut child = Command::new("cargo")
        .arg("run")
        .arg("--manifest-path")
        .arg("../server/Cargo.toml")
        .arg("--")
        .arg("--port")
        .arg(command_port.get_path().as_os_str())
        .arg("--data-folder")
        .arg("../test_data")
        .spawn()
        .unwrap();
    select! {
        result = child.wait() => {
            println!("Child finished with result: {:?}", result);
        }
        _ = receiver => {
            if let Some(pid) = child.id() {
                println!("Shutting down child...");
                Command::new("kill").arg("-2").arg(pid.to_string()).spawn().unwrap().wait().await.unwrap();
                println!("Child exitted.");
            }
        }
    }
}
