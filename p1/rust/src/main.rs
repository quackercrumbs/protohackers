use primes;
use serde::{Deserialize, Serialize};

use console_subscriber;
use tokio::io;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::net;
use tokio::sync;
use tracing::{info, instrument};
use tracing_subscriber::prelude::*;

// leave a comment here
fn process_request(request: &Request) -> Result<Response, String> {
    if request.method != "isPrime" {
        Err(String::from("Method is not isPrime"))
    } else if let Some(number) = request.number.as_u64() {
        let result = primes::is_prime(number);
        Ok(Response {
            method: String::from("isPrime"),
            prime: result,
        })
    } else {
        // Its either a floating point number or negative
        Ok(Response {
            method: String::from("isPrime"),
            prime: false,
        })
    }
}

#[instrument]
async fn process(mut socket: net::TcpStream) {
    info!("processing {:?}", socket.peer_addr());
    let (read_half, mut write_half) = socket.split();
    let reader = io::BufReader::new(read_half);
    let mut lines = reader.lines();
    // TODO: convert to using .map or for .. in ..?
    while let Ok(Some(request_raw)) = lines.next_line().await {
        info!("New Line: {:?}", request_raw);
        let request: Request = if let Ok(request) = serde_json::from_str(&request_raw) {
            request
        } else {
            info!("Malformed response, bad serialization {:?}", request_raw);
            // request is malformed during serialization
            write_half
                .write_all(
                    serde_json::to_string(&MalformedResponse {})
                        .expect("Couldn't serialize malformed response")
                        .as_bytes(),
                )
                .await
                .expect("Couldn't write malformed response");
            write_half
                .write_all("\n".as_bytes())
                .await
                .expect("Couldn't write newline");
            write_half.flush().await.expect("Couldn't flush socket");
            write_half
                .shutdown()
                .await
                .expect("Could not shutdown socket");
            return;
        };
        info!("parsed request {:?}", request);

        let result = process_request(&request);
        if let Ok(response) = result {
            info!("response: {:?}", response);
            // write back to client
            write_half
                .write_all(
                    serde_json::to_string(&response)
                        .expect("Couldn't serialize response")
                        .as_bytes(),
                )
                .await
                .expect("Couldn't write response");
            info!("response write all: done");
            write_half
                .write_all("\n".as_bytes())
                .await
                .expect("Couldn't write newline");
            info!("response write newline: done");
            write_half.flush().await.expect("Couldn't flush socket");
            info!("response write flush: done");
        } else {
            // send back malformed response and close client
            info!("Malformed response, unprocessable {:?}", request);
            write_half
                .write_all(
                    serde_json::to_string(&MalformedResponse {})
                        .expect("Couldn't serialize malformed response")
                        .as_bytes(),
                )
                .await
                .expect("Couldn't write malformed response");
            write_half
                .write_all("\n".as_bytes())
                .await
                .expect("Couldn't write newline");
            write_half.flush().await.expect("Couldn't flush socket");
            write_half
                .shutdown()
                .await
                .expect("Could not shutdown socket");
            info!("Shutdown write_half");
            break;
        }
    }
    info!("No more lines, exited loop");
}

#[instrument]
async fn serve_async(ready_tx: sync::oneshot::Sender<bool>) {
    let listener = net::TcpListener::bind("0.0.0.0:8000")
        .await
        .expect("Unable to bind to TCP Address to listen.");
    ready_tx.send(true).expect("Unable to send ready signal");
    loop {
        info!("Waiting for connection");
        let (socket, _) = listener.accept().await.unwrap();
        let socket_addr = socket.peer_addr();
        info!("Accepted for socket {:?}", socket_addr);
        tokio::spawn(async move {
            process(socket).await;
            println!("Finished for socket {:?}", socket_addr);
        });
    }
}

#[tokio::main]
#[instrument]
async fn main() {
    let console_layer = console_subscriber::spawn();
    let (ready_tx, _ready_rx) = sync::oneshot::channel();
    tracing_subscriber::registry()
        .with(console_layer)
        .with(tracing_subscriber::fmt::layer())
        .init();
    // tracing_subscriber::fmt::init();
    serve_async(ready_tx).await;
}

#[derive(Debug, Deserialize)]
struct Request {
    method: String,
    number: serde_json::value::Number,
}

#[derive(Debug, Serialize)]
struct Response {
    method: String,
    prime: bool,
}

#[derive(Debug, Serialize)]
struct MalformedResponse {}

#[cfg(test)]
mod integration_tests {

    use super::*;

    #[test]
    fn test_server() {
        // run this to see logs:
        // cargo test server -- --nocapture
        let console_layer = console_subscriber::spawn();
        tracing_subscriber::registry()
            .with(console_layer)
            .with(tracing_subscriber::fmt::layer().with_filter(
                    // TRACE is a bit chatty, set it here if you want it
                tracing_subscriber::filter::LevelFilter::from_level(tracing::Level::INFO),
            ))
            .init();

        // ready signal
        let (ready_tx, ready_rx) = sync::oneshot::channel();

        let rt = tokio::runtime::Runtime::new().expect("Unable to create tokio runtime for test.");
        rt.spawn(async {
            info!("Spawned test server.");
            serve_async(ready_tx).await;
            info!("test server shutdown.");
        });

        rt.block_on(async {
            // wait for server to be ready
            ready_rx
                .await
                .expect("Failure while waiting for ready signal");

            // send request to server running
            let socket = tokio::net::TcpSocket::new_v4().unwrap();
            let address = "127.0.0.1:8000".parse().unwrap();
            info!("Attempting to connect to {:?}", address);
            let mut stream = socket
                .connect(address)
                .await
                .expect("Couldn't connect to test server");
            stream
                .write_all(b"{\"method\":\"isPrime\",\"number\":10}")
                .await
                .expect("Couldn't write to test socket");
            stream.flush().await.expect("Couldn't flush test socket");
            info!("Written to stream.");
            stream
                .shutdown()
                .await
                .expect("Couldn't shutdown write side of test socket");
            info!("Close stream");

            let reader = io::BufReader::new(stream);
            let mut response_buffer = reader.lines();
            let response = response_buffer
                .next_line()
                .await
                .expect("There is no response data");

            info!("Completed response retrieval");

            assert_eq!(Some(String::from("{\"method\":\"isPrime\",\"prime\":false}")), response);
        });
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_process_request_happy() {
        let request = Request {
            method: "isPrime".into(),
            number: serde_json::value::Number::from(10),
        };
        let result = process_request(&request);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().prime, false);

        let request = Request {
            method: "isPrime".into(),
            number: serde_json::value::Number::from(13),
        };
        let result = process_request(&request);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().prime, true);

        let request = Request {
            method: "isPrime".into(),
            number: serde_json::value::Number::from(-13),
        };
        let result = process_request(&request);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().prime, false);

        let request = Request {
            method: "isPrime".into(),
            number: serde_json::value::Number::from_f64(13.0)
                .expect("Could not create f64 for number"),
        };
        let result = process_request(&request);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().prime, false);
    }

    #[test]
    fn test_process_request_malformed() {
        let request = Request {
            method: "invalidMethod".into(),
            number: serde_json::value::Number::from(10),
        };
        let result = process_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_primes() {
        assert!(primes::is_prime(13));
    }

    #[test]
    fn test_serde_positive_whole_number() {
        let request_str = "{\"method\":\"isPrime\",\"number\":10}";
        let request_deserialized: Request =
            serde_json::from_str(&request_str).expect("Could not deserialize str");
        // can be both unsigned and signed
        assert!(request_deserialized.number.is_u64());
        assert!(request_deserialized.number.is_i64());
    }

    #[test]
    fn test_serde_negative_whole_number() {
        let request_str = "{\"method\":\"isPrime\",\"number\":-10}";
        let request_deserialized: Request =
            serde_json::from_str(&request_str).expect("Could not deserialize str");
        // has to be signed
        assert!(request_deserialized.number.is_i64());
        // can't be unsigned
        assert!(!request_deserialized.number.is_u64());
    }

    #[test]
    fn test_serde_positive_float_number() {
        let request_str = "{\"method\":\"isPrime\",\"number\":10.0}";
        let request_deserialized: Request =
            serde_json::from_str(&request_str).expect("Could not deserialize str");
        assert!(request_deserialized.number.is_f64());
    }

    #[test]
    fn test_serde_negative_float_number() {
        let request_str = "{\"method\":\"isPrime\",\"number\":-10.0}";
        let request_deserialized: Request =
            serde_json::from_str(&request_str).expect("Could not deserialize str");
        assert!(request_deserialized.number.is_f64());
    }
}
