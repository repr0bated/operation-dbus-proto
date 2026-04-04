use anyhow::Result;
use op_dbus_mirror::object::MirrorObject;
use simd_json::json;
use std::time::Instant;
use tracing_subscriber::EnvFilter;
use zbus::connection::Builder;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    let count = 16_000;
    println!(
        "🚀 Starting D-Bus performance verification with {} objects...",
        count
    );

    let conn = Builder::session()?
        .name("org.opdbus.mirror.perf_test")?
        .build()
        .await?;

    let start = Instant::now();

    for i in 0..count {
        let path = format!("/org/opdbus/mirror/perf/obj_{}", i);
        let dbus_path = zbus::zvariant::ObjectPath::try_from(path)?;

        let data = json!({
            "id": i,
            "uuid": format!("uuid_{}", i),
            "status": "active",
            "metadata": {
                "created_at": "2026-02-12T00:00:00Z",
                "owner": "perf-test",
                "tags": ["test", "performance", "heavy-load"]
            }
        });

        let obj = MirrorObject::new(data.into());
        conn.object_server().at(dbus_path, obj).await?;

        if (i + 1) % 1000 == 0 {
            println!("   Registered {}/{} objects...", i + 1, count);
        }
    }

    let duration = start.elapsed();
    let per_object = duration.as_micros() / count as u128;

    println!("\n✅ Performance Results:");
    println!("   Total Objects:    {}", count);
    println!("   Total Time:       {:?}", duration);
    println!("   Avg Per Object:   {} us", per_object);

    // Give zbus a moment to settle
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    #[zbus::proxy(
        interface = "org.opdbus.MirrorObjectV1",
        default_service = "org.opdbus.mirror.perf_test"
    )]
    trait MirrorObject {
        fn get_json(&self) -> zbus::Result<String>;
    }

    let test_path = "/org/opdbus/mirror/perf/obj_8000";
    let lookup_start = Instant::now();

    let proxy = MirrorObjectProxy::builder(&conn)
        .path(test_path)?
        .build()
        .await?;

    let reply = proxy.get_json().await?;

    let lookup_duration = lookup_start.elapsed();
    println!("   Single Lookup:    {:?} (obj_8000)", lookup_duration);

    if reply.contains("uuid_8000") {
        println!("   Data Integrity:   PASS");
    } else {
        println!("   Data Integrity:   FAIL");
    }

    println!("\nVerification complete. Press Ctrl+C to exit.");
    // Exit after a few seconds to avoid hanging CI-like runs
    tokio::select! {
        _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {},
        _ = tokio::signal::ctrl_c() => {},
    }

    Ok(())
}
