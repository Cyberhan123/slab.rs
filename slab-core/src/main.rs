mod routes;
mod handlers;
mod api;
mod services;

use axum;
use tokio;


#[tokio::main]
async fn main() {
     // initialize tracing
    tracing_subscriber::fmt::init();

    // build our application with a route
    let app = routes::app();
    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    let _result = axum::serve(listener, app).await;
}