#[macro_use]
extern crate trackable;

// sudo apt-get install pkg-config libssl-dev 

use clap::Parser;
use std::thread;
use rusturn::auth::AuthParams;
use trackable::error::MainError;
use fibers_global;

// From https://github.com/rasviitanen/rustysignal/blob/master/src/main.rs
mod server;
mod node;
mod network;


#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[clap(long, default_value_t = 3478)]
    websocket_signaling_port: u16,
    
    #[clap(long, action, default_value_t = false)]
    websocket_use_ssl: bool,

    #[clap(long)]
    websocket_ssl_cert_path: Option<String>,
    
    #[clap(long)]
    websocket_ssl_cert_key_path: Option<String>,

    #[clap(long, default_value_t = 3479)]
    turn_port: u16,
    
    /// Username.
    #[clap(long, default_value = "foo")]
    turn_username: String,

    /// Password.
    #[clap(long, default_value = "bar")]
    turn_password: String,

    /// Realm.
    #[clap(long, default_value = "baz")]
    turn_realm: String,

    /// Nonce.
    #[clap(long, default_value = "qux")]
    turn_nonce: String,

    #[clap(long, default_value_t = 3480)]
    stun_port: u16,
}

fn run_turn_server(port: u16, username: String, password: String, realm: String, nonce: String) -> Result<(), MainError> {
    println!("running turn server on port {}", port);
    let addr = track_any_err!(format!("0.0.0.0:{}", port).parse())?;
    let auth_params = track!(AuthParams::with_realm_and_nonce(
        &username,
        &password,
        &realm,
        &nonce
    ))?;

    let turn_server = track!(fibers_global::execute(rusturn::server::UdpServer::start(
        addr,
        auth_params,
    )))?;
    
    track!(fibers_global::execute(turn_server))?;

    Ok(())
}

fn run_stun_server(port: u16) -> Result<(), MainError> {
    println!("running stun server on port {}", port);
    let addr = track_any_err!(format!("0.0.0.0:{}", port).parse())?;
    let server = track!(fibers_global::execute(rustun::server::UdpServer::start(
        fibers_global::handle(),
        addr,
        rustun::server::BindingHandler
    )))?;
    track!(fibers_global::execute(server))?;
    Ok(())
}

fn run_websocket_signaling_server(port: u16, use_ssl: bool, ssl_cert_path: Option<String>, ssl_cert_key_path: Option<String>) {
    server::run(port, use_ssl, ssl_cert_path, ssl_cert_key_path)
}

fn main() -> Result<(), MainError>{
    let args = Args::parse();
    let stun_handle = thread::spawn(move || {
        let _ = run_stun_server(args.stun_port);
    });

    let turn_handle = thread::spawn(move || {
        let _ = run_turn_server(args.turn_port, args.turn_username, args.turn_password, args.turn_realm, args.turn_nonce);
    });
    
    let websocket_signaling_handle = thread::spawn(move || {
        run_websocket_signaling_server(args.websocket_signaling_port, args.websocket_use_ssl, args.websocket_ssl_cert_path, args.websocket_ssl_cert_key_path)
    });

    websocket_signaling_handle.join().unwrap();
    
    stun_handle.join().unwrap();
    turn_handle.join().unwrap();
    Ok(())
}