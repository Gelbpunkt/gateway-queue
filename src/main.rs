use hyper::{
    body::Body,
    server::{conn::AddrStream, Server},
    service, Error as HyperError, Request, Response,
};
use log::{debug, error, info};
use std::{
    convert::TryInto,
    env,
    error::Error,
    net::{IpAddr, SocketAddr},
    str::FromStr,
    sync::Arc,
};
use twilight_gateway_queue::{LargeBotQueue, LocalQueue, Queue};
use twilight_http::Client;

const PROCESSED: &[u8] = br#"{"message": "You're free to connect now! :)"}"#;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }
    pretty_env_logger::try_init_timed()?;

    let host_raw = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let host = IpAddr::from_str(&host_raw)?;
    let port = env::var("PORT").unwrap_or_else(|_| "80".into()).parse()?;

    let queue: Arc<Box<dyn Queue>> = {
        if let Ok(token) = env::var("DISCORD_TOKEN") {
            let http_client = Client::new(token);
            let gateway = http_client
                .gateway()
                .authed()
                .await
                .expect("Cannot fetch gateway information");
            Arc::new(Box::new(
                LargeBotQueue::new(
                    gateway
                        .session_start_limit
                        .max_concurrency
                        .try_into()
                        .unwrap(),
                    &http_client,
                )
                .await,
            ))
        } else {
            Arc::new(Box::new(LocalQueue::new()))
        }
    };

    let address = SocketAddr::from((host, port));

    // The closure inside `make_service_fn` is run for each connection,
    // creating a 'service' to handle requests for that specific connection.
    let service = service::make_service_fn(move |addr: &AddrStream| {
        debug!("Connection from: {:?}", addr);
        let queue = queue.clone();

        async move {
            Ok::<_, HyperError>(service::service_fn(move |_: Request<Body>| {
                let queue = queue.clone();

                async move {
                    queue.request([0, 0]).await;

                    let body = Body::from(PROCESSED.to_vec());

                    Ok::<Response<Body>, HyperError>(Response::new(body))
                }
            }))
        }
    });

    let server = Server::bind(&address).serve(service);

    info!("Listening on http://{}", address);

    if let Err(why) = server.await {
        error!("Fatal server error: {}", why);
    }

    Ok(())
}
