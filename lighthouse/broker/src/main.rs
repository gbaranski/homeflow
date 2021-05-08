use actix_web::{get, http, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use actix_web_actors::ws;
use houseflow_types::{DeviceID, DevicePassword};
use itertools::Itertools;
use lighthouse_api::Request;
use lighthouse_proto::execute;
use session::Session;
use std::collections::HashMap;
use std::str::FromStr;
use tokio::sync::Mutex;

mod session;

fn parse_authorization_header(req: &HttpRequest) -> Result<(DeviceID, DevicePassword), String> {
    let header = req
        .headers()
        .get(http::header::AUTHORIZATION)
        .ok_or(String::from("`Authorization` header is missing"))?
        .to_str()
        .map_err(|err| format!("Invalid string `Authorization` header, error: `{}`", err))?;

    let mut iter = header.split_whitespace();
    let auth_type = iter
        .next()
        .ok_or("Missing auth type in `Authorization` header")?;
    if auth_type != "Basic" {
        return Err(format!("Invalid auth type: {}", auth_type));
    }
    let credentials = iter
        .next()
        .ok_or("Missing credentials in `Authorization` header")?;

    let (device_id, device_password) = credentials
        .split_terminator(":")
        .take(2)
        .next_tuple()
        .ok_or("Missing ID/Password in `Authorization` header")?;

    Ok((
        DeviceID::from_str(device_id).map_err(|err| err.to_string())?,
        DevicePassword::from_str(device_password).map_err(|err| err.to_string())?,
    ))
}

#[get("/ws")]
async fn on_websocket(
    req: HttpRequest,
    stream: web::Payload,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let address = req.peer_addr().unwrap();
    let (device_id, device_password) = match parse_authorization_header(&req) {
        Ok(v) => v,
        Err(err) => {
            return Ok::<HttpResponse, actix_web::Error>(HttpResponse::BadRequest().body(err))
        } // TODO: Consider changing Ok to Err
    };
    log::debug!(
        "DeviceID: {}, DevicePassword: {}",
        device_id,
        device_password
    );
    let session = Session::new(device_id.clone(), address);
    let (address, response) = ws::start_with_addr(session, &req, stream).unwrap();
    app_state.sessions.lock().await.insert(device_id, address);
    log::debug!("Response: {:?}", response);
    Ok(response)
}

#[get("/test")]
async fn on_test(_req: HttpRequest, app_state: web::Data<AppState>) -> impl Responder {
    let params = r#"
    {
        "on": true,
        "openPercent": 80
    }
        "#;
    let params = serde_json::from_str(params).expect("invalid params");
    let frame = execute::Frame::new(execute::Command::OnOff, params);
    let request = Request::Execute(frame);
    let response = app_state
        .sessions
        .lock()
        .await
        .values()
        .nth(0)
        .unwrap()
        .send(request)
        .await
        .unwrap()
        .unwrap();
    log::debug!("Response: {:?}", response);
    HttpResponse::Ok().body("received")
}

struct AppState {
    sessions: Mutex<HashMap<DeviceID, actix::Addr<Session>>>,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    let addr = "127.0.0.1:8080";
    let app_state = web::Data::new(AppState {
        sessions: Default::default(),
    });
    let server = HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .service(on_websocket)
            .service(on_test)
    })
    .bind(&addr)?;

    server.run().await
}
