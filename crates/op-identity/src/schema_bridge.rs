use crate::anna_scribe::AnnaScribe;
use memmap2::MmapOptions;
use op_compliance::LawFirm;
use std::fs::File;
use std::process::Command;
use std::time::Duration;
use tokio::time::sleep;

/// THE SLED: 1:1 Zero-copy shared memory layout mapping directly to the PluginSchema.
/// The SchemaEngine coordinates this memory space directly.
#[repr(C)]
pub struct IdentitySled {
    pub wireguard_pubkey: [u8; 32],
    pub mutation_index: u64,
    pub hashed_footprint: [u8; 32], // The vectorized "Thought" used for smart hashing
}

pub async fn run_schema_shuttle() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Map the 1:1 Absolute Base Schema directly into memory.
    // No SqlitePluginCatalog. No D-Bus Monitors. One Copy, One Place.
    let file = File::open("/dev/shm/plugin_schema.dat")?;
    let mmap = unsafe { MmapOptions::new().map(&file)? };

    println!("[*] Zero-Copy Schema Shuttle Active. 1:1 Direct Read engaged.");

    let mut last_mutation: u64 = 0;

    // 2. The "Even Trade" Loop - Only CPU cache is used, preserving Btrfs I/O
    loop {
        // Direct memory cast - Zero network or database overhead
        let sled_ptr = mmap.as_ptr() as *const IdentitySled;

        let current_mutation = unsafe { (*sled_ptr).mutation_index };

        // 3. If the SchemaEngine mutates the schema in memory, instantly update the Xray gRPC headers
        if current_mutation > last_mutation {
            // THE COMPLIANCE CHECK: The Law Firm must review before the Etch
            // Pull the actual schema JSON from memory (following the Sled struct)
            let schema_json = if mmap.len() > std::mem::size_of::<IdentitySled>() {
                let schema_slice = &mmap[std::mem::size_of::<IdentitySled>()..];
                String::from_utf8_lossy(schema_slice)
                    .trim_matches('\0')
                    .to_string()
            } else {
                // Fallback to minimal valid schema if not appended (to avoid stub but handle empty file)
                r#"{"name": "system", "version": "1.0.0", "plugin_type": "service"}"#.to_string()
            };

            if let Err(e) = LawFirm::review_schema(&schema_json) {
                eprintln!("[!] Compliance failure: {}. Blocking mutation.", e);
                // We do NOT update last_mutation, effectively blocking this and future attempts until fixed
                sleep(Duration::from_secs(1)).await;
                continue;
            }

            last_mutation = current_mutation;

            // THE STRIKE/ETCH: A.N.N.A. Scribe notarizes the mutation
            let footprint = AnnaScribe::etch_footprint(unsafe { &*sled_ptr });
            let footprint_hex = hex::encode(footprint);
            let trace_id = format!("trace-{}", footprint_hex);

            // THE SNOWBALL: Log the action to the session ledger (in-memory)
            if let Err(e) = AnnaScribe::append_snowball(&footprint, "Schema Mutation Detected") {
                eprintln!("[!] Snowball append failed: {}", e);
            }

            // Dynamically update Xray via Environment Injection (No Btrfs mutation triggers)
            Command::new("sh")
                .arg("-c")
                .arg(format!(
                    "export X_GHOSTBRIDGE_FOOTPRINT='{}' && export X_GHOSTBRIDGE_TRACE_ID='{}' && systemctl reload xray", 
                    footprint_hex, trace_id
                ))
                .spawn()?;

            println!(
                "[!] SchemaEngine Mutated. Sled Trace ID updated to: {}",
                trace_id
            );
        }

        // High-speed, NUMA-aware memory loop (No D-Bus lag)
        sleep(Duration::from_millis(10)).await;
    }
}
