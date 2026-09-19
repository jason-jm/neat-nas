//! Probe an SMB server: negotiate, authenticate, list shares.
//!
//!   cargo run --example probe -- 192.168.1.126:445 USER PASS
//!
//! With RUST_LOG=smb2=info the negotiated dialect and signing/encryption
//! state are printed even when authentication fails.

use neatnas_lib::smb::{self, ConnectionParams, Session};

#[tokio::main]
async fn main() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("smb2=info")).try_init();
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("usage: probe HOST:PORT USER PASS");
        std::process::exit(2);
    }
    let (host, port) = args[1].rsplit_once(':').expect("HOST:PORT");
    let params = ConnectionParams {
        host: host.to_string(),
        port: port.parse().expect("port"),
        username: args[2].clone(),
        password: args[3].clone(),
        domain: args.get(4).cloned().unwrap_or_default(),
    };
    let started = std::time::Instant::now();
    match smb::connect(&params).await {
        Ok(client) => {
            let mut session = Session::new(client);
            println!("connected in {:?}, dialect {}", started.elapsed(), session.dialect());
            match session.list_shares().await {
                Ok(shares) => {
                    println!("{} disk shares:", shares.len());
                    for s in &shares {
                        println!("  {}  {}", s.name, s.comment);
                    }
                    if let Some(first) = shares.first() {
                        match session.list_dir(&first.name, "").await {
                            Ok(entries) => println!("root of {}: {} entries", first.name, entries.len()),
                            Err(e) => println!("list_dir {} failed: {e}", first.name),
                        }
                    }
                }
                Err(e) => println!("list_shares failed: {e}"),
            }
        }
        Err(e) => {
            println!("connect failed after {:?}: code={} message={}", started.elapsed(), e.code, e.message);
            std::process::exit(1);
        }
    }
}
