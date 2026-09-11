use std::{
    io,
    net::{Shutdown, TcpListener, TcpStream},
    path::Path,
};

use crate::{
    config::Config,
    handler,
    http::{read_request, HttpResponse, RequestLimits},
    path_security::canonicalize_document_root,
};

pub fn run(config: Config) -> io::Result<()> {
    let canonical_root = canonicalize_document_root(&config.document_root)?;
    let listener = TcpListener::bind(&config.address)?;
    let local_address = listener.local_addr()?;
    eprintln!(
        "listening on http://{} with document root {}",
        local_address,
        canonical_root.display()
    );

    // This loop is intentionally sequential: accept one client, finish its one
    // request and response, close it, and only then accept the next client.
    for incoming in listener.incoming() {
        match incoming {
            Ok(mut stream) => {
                if let Err(error) =
                    handle_connection(&mut stream, &config, &canonical_root, local_address.port())
                {
                    eprintln!("client connection error: {error}");
                }
                let _ = stream.shutdown(Shutdown::Both);
            }
            Err(error) => eprintln!("accept error: {error}"),
        }
    }
    Ok(())
}

fn handle_connection(
    stream: &mut TcpStream,
    config: &Config,
    canonical_root: &Path,
    server_port: u16,
) -> io::Result<()> {
    stream.set_read_timeout(Some(config.read_timeout))?;
    stream.set_write_timeout(Some(config.write_timeout))?;
    let peer = stream.peer_addr()?;
    let limits = RequestLimits::new(config.max_body_size);

    let request = match read_request(stream, limits) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("invalid request from {}: {error}", peer.ip());
            return HttpResponse::error(error.status()).write_to(stream, false);
        }
    };

    let response =
        handler::handle_request(&request, config, canonical_root, peer.ip(), server_port);
    let omit_body = request.method.is_head();
    let status = response.status.code();
    let response_length = response.body.len();
    let write_result = response.write_to(stream, omit_body);

    println!(
        "{} \"{} {} {}\" {} {}",
        peer.ip(),
        request.method.as_str(),
        request.request_target,
        request.version,
        status,
        response_length
    );
    write_result
}
