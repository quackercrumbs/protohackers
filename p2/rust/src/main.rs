use tokio::io::{AsyncRead, AsyncReadExt};
use tracing::dispatcher::DefaultGuard;
use tracing::{debug, info, error};
use tracing_subscriber::prelude::*;

fn main() {
    let _guard = setup_tracing();
    info!("Hello, world!");
}

/**
 * Once the guard is dropped, the global default is reset.
 * This helps with testing because there can only be 1 global default at a time.
 * Using `init()` will fail, so that's why we use `set_default()`
 */
fn setup_tracing() -> DefaultGuard {
    let guard = tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .set_default();
    info!("Tracing has been setup");
    guard
}

// Time, Price
struct PricePoint(i32, i32);

fn handle_insert(storage: &mut Vec<PricePoint>, point: PricePoint) {
    storage.push(point);
}

struct QueryRange {
    start: i32,
    end: i32,
}

fn handle_avg_query(storage: &Vec<PricePoint>, query: QueryRange) -> i32 {
    if query.start > query.end {
        return 0;
    }

    let result = storage
        .iter()
        .filter(|price_point| price_point.0 >= query.start && price_point.0 <= query.end)
        .fold((0, 0), |acc, price_point| {
            (acc.0 + 1, acc.1 + price_point.1)
        });
    let count = result.0;
    if count == 0 {
        return 0;
    } else {
        result.1 / count
    }
}

async fn read_message(stream: &mut (impl AsyncRead + std::marker::Unpin)) -> (char, i32, i32) {
    // for the read stream, read the 9 bytes
    let message_type = stream.read_u8().await.unwrap();
    let field_1 = stream.read_i32().await.unwrap();
    let field_2 = stream.read_i32().await.unwrap();

    (char::from(message_type), field_1, field_2)
}

use tokio::net::TcpListener;
use tokio::sync::oneshot;

async fn serve(ready_signal: oneshot::Sender<bool>) {
    let listener = TcpListener::bind("0.0.0.0:8000").await.expect("Couldn't start tcp listener on addres");
    info!("Listening on address: {:?}", listener.local_addr());
    ready_signal.send(true).expect("Couldn't send ready signal after server has started");

    loop {
        let stream = listener.accept().await;
        match stream {
            Ok(stream) => {
                info!("Accepted connection for {:?}", stream);
            }
            Err(e) => {
                error!("Error when listening for connection, {:?}", e);
            }
        };
    }
}

#[cfg(test)]
mod server_tests {

    use super::*;

    #[tokio::test]
    async fn test_server_startup() {
        let _guard = setup_tracing();
        let (ready_sender, ready_receiver) = oneshot::channel();
        tokio::spawn(async {
            serve(ready_sender).await;
        });

        let ready_signal = ready_receiver.await;
        assert_eq!(Ok(true), ready_signal);
    }

}

#[cfg(test)]
mod parsing_tests {

    use super::*;

    use std::io::Cursor;

    #[tokio::test]
    async fn test_parsing() {
        let _guard = setup_tracing();
        let mut reader = Cursor::new(vec![
            0x51, // Q
            0x00, 0x00, 0x00, 0x01, // 1
            0x00, 0x00, 0x00, 0x02, // 2
        ]);
        let result = read_message(&mut reader).await;
        info!("results = {:?}", result);
        assert_eq!(('Q', 1, 2), result);
    }
}

#[cfg(test)]
mod storage_tests {

    use super::*;

    #[tokio::test]
    async fn test() {
        let _guard = setup_tracing();

        {
            // inclusive on edges
            let mut storage: Vec<PricePoint> = Vec::new();
            handle_insert(&mut storage, PricePoint(1, 100));
            handle_insert(&mut storage, PricePoint(0, 0));
            let avg = handle_avg_query(&storage, QueryRange { start: 0, end: 1 });
            assert_eq!(50, avg);
        }

        {
            // ignore outside range
            let mut storage: Vec<PricePoint> = Vec::new();
            handle_insert(&mut storage, PricePoint(1, 100));
            handle_insert(&mut storage, PricePoint(2, 0));
            let avg = handle_avg_query(&storage, QueryRange { start: 0, end: 1 });
            assert_eq!(100, avg);
        }

        {
            // happy path
            let mut storage: Vec<PricePoint> = Vec::new();
            handle_insert(&mut storage, PricePoint(1, 1));
            handle_insert(&mut storage, PricePoint(2, 2));
            handle_insert(&mut storage, PricePoint(3, 3));
            handle_insert(&mut storage, PricePoint(4, 4));
            let avg = handle_avg_query(&storage, QueryRange { start: 0, end: 4 });
            assert_eq!(2, avg);
        }

        {
            // fractional
            let mut storage: Vec<PricePoint> = Vec::new();
            handle_insert(&mut storage, PricePoint(1, 1));
            handle_insert(&mut storage, PricePoint(2, 2));
            handle_insert(&mut storage, PricePoint(2, 2));
            let avg = handle_avg_query(&storage, QueryRange { start: 0, end: 2 });
            assert_eq!(1, avg);
        }

        {
            // fractional + negative
            let mut storage: Vec<PricePoint> = Vec::new();
            handle_insert(&mut storage, PricePoint(1, -1));
            handle_insert(&mut storage, PricePoint(2, -2));
            handle_insert(&mut storage, PricePoint(2, -2));
            let avg = handle_avg_query(&storage, QueryRange { start: 0, end: 2 });
            assert_eq!(-1, avg);
        }

        {
            // no inserts
            let storage: Vec<PricePoint> = Vec::new();
            let avg = handle_avg_query(&storage, QueryRange { start: 0, end: 2 });
            assert_eq!(0, avg);
        }

        {
            // no elements in range
            let mut storage: Vec<PricePoint> = Vec::new();
            handle_insert(&mut storage, PricePoint(1, 1));
            handle_insert(&mut storage, PricePoint(2, 2));
            let avg = handle_avg_query(
                &storage,
                QueryRange {
                    start: 100,
                    end: 2000,
                },
            );
            assert_eq!(0, avg);
        }

        {
            // start > end, which is invalid
            let mut storage: Vec<PricePoint> = Vec::new();
            handle_insert(&mut storage, PricePoint(1, 1));
            handle_insert(&mut storage, PricePoint(2, 2));
            let avg = handle_avg_query(&storage, QueryRange { start: 200, end: 1 });
            assert_eq!(0, avg);
        }
    }
}
