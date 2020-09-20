use actix_web::{middleware, web, App, HttpRequest, HttpServer, Responder};

mod cec;
use cec::CECActor;
async fn greet(req: HttpRequest) -> impl Responder {
    let name = req.match_info().get("name").unwrap_or("World");
    format!("Hello {}!", &name)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let cecActor = CECActor::start();

    HttpServer::new(|| {
        App::new()
            .data(cecActor)
            .wrap(middleware::Logger::default())
            .route("/", web::get().to(greet))
            .route("/{name}", web::get().to(greet))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await

    // todo
    // read vol
    // need to read up on google home graph
    // allow mute
    // for now, curl to test out each?
    //https://developers.google.com/assistant/smarthome/develop/create#setup-server
}
