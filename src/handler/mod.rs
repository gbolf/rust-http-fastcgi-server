pub mod static_files;

use std::{net::IpAddr, path::Path};

use crate::{
    config::Config,
    fastcgi::{self, FastCgiError, FastCgiRequest},
    http::{HttpRequest, HttpResponse, Method, StatusCode},
    path_security::{resolve_path, PathError},
};

pub fn handle_request(
    request: &HttpRequest,
    config: &Config,
    canonical_root: &Path,
    remote_ip: IpAddr,
    server_port: u16,
) -> HttpResponse {
    match &request.method {
        Method::KnownUnsupported(_) => return method_not_allowed(),
        Method::Unknown(_) => return HttpResponse::error(StatusCode::NotImplemented),
        Method::Get | Method::Head | Method::Post => {}
    }

    let resolved = match resolve_path(canonical_root, &request.path) {
        Ok(path) => path,
        Err(PathError::BadEncoding) => return HttpResponse::error(StatusCode::BadRequest),
        Err(PathError::Forbidden) => return HttpResponse::error(StatusCode::Forbidden),
        Err(PathError::NotFound) => return HttpResponse::error(StatusCode::NotFound),
        Err(PathError::Io(error)) => {
            eprintln!("path resolution error: {error}");
            return HttpResponse::error(StatusCode::InternalServerError);
        }
    };

    let is_php = resolved
        .filesystem_path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("php"));
    if is_php {
        let remote_address = remote_ip.to_string();
        let server_port = server_port.to_string();
        let fastcgi_request = FastCgiRequest {
            http: request,
            script_filename: &resolved.filesystem_path,
            document_root: canonical_root,
            remote_address: &remote_address,
            server_port: &server_port,
        };
        return match fastcgi::execute(
            &config.php_fpm,
            fastcgi_request,
            config.read_timeout,
            config.write_timeout,
        ) {
            Ok(response) => response,
            Err(FastCgiError::Timeout) => {
                eprintln!("PHP-FPM communication timed out");
                HttpResponse::error(StatusCode::GatewayTimeout)
            }
            Err(error) => {
                eprintln!("PHP-FPM error: {error}");
                HttpResponse::error(StatusCode::BadGateway)
            }
        };
    }

    if matches!(request.method, Method::Post) {
        return method_not_allowed();
    }

    match static_files::serve(&resolved.filesystem_path) {
        Ok(response) => response,
        Err(error) => {
            eprintln!("static file read error: {error}");
            HttpResponse::error(StatusCode::InternalServerError)
        }
    }
}

fn method_not_allowed() -> HttpResponse {
    let mut response = HttpResponse::error(StatusCode::MethodNotAllowed);
    response
        .headers
        .push(("Allow".to_string(), "GET, HEAD, POST".to_string()));
    response
}
