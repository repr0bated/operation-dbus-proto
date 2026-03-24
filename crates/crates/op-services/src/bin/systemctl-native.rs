//! Native systemctl - D-Bus client (no network dependency)

use std::env;
use zbus::Connection;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    let conn = Connection::system().await?;
    let proxy = zbus::Proxy::new(
        &conn,
        "org.opdbus.services",
        "/org/opdbus/services",
        "org.opdbus.services.v1.Manager",
    )
    .await?;

    match args[1].as_str() {
        "start" => {
            let name = args
                .get(2)
                .ok_or_else(|| anyhow::anyhow!("missing service name"))?;
            let result: String = proxy.call("Start", &(name.as_str(),)).await?;
            println!("Started {}", name);
        }
        "stop" => {
            let name = args
                .get(2)
                .ok_or_else(|| anyhow::anyhow!("missing service name"))?;
            let _: String = proxy.call("Stop", &(name.as_str(),)).await?;
            println!("Stopped {}", name);
        }
        "restart" => {
            let name = args
                .get(2)
                .ok_or_else(|| anyhow::anyhow!("missing service name"))?;
            let _: String = proxy.call("Restart", &(name.as_str(),)).await?;
            println!("Restarted {}", name);
        }
        "status" => {
            let name = args
                .get(2)
                .ok_or_else(|| anyhow::anyhow!("missing service name"))?;
            let result: String = proxy.call("GetStatus", &(name.as_str(),)).await?;
            println!("● {}", name);
            println!("{}", result);
        }
        "list-units" | "list" => {
            let services: Vec<String> = proxy.call("ListServices", &()).await?;
            for svc in services {
                println!("{}", svc);
            }
        }
        _ => print_usage(),
    }

    Ok(())
}

fn print_usage() {
    eprintln!("Usage: systemctl <command> [service]");
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  start <service>    Start a service");
    eprintln!("  stop <service>     Stop a service");
    eprintln!("  restart <service>  Restart a service");
    eprintln!("  status <service>   Show service status");
    eprintln!("  list-units         List all services");
}
