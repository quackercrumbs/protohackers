use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};

fn handle_client(stream: &mut TcpStream) {
    //println!("hello connection");
    // read until stream closes send side
    let mut buffer = Vec::new(); 
    let result = stream.read_to_end(&mut buffer);
    // println!("read {:?}", result);

    // then write to stream
    // println!("data {:?}", String::from_utf8(buffer.clone()));
    let result = stream.write_all(&buffer);
    // println!("result = {:?}", result);
    let result = stream.flush();
    // println!("result = {:?}", result);

    // send close signal to stream
    // println!("Good bye");
    let result = stream.shutdown(std::net::Shutdown::Both);
    // println!("Result = {:?}", result);
}

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:8000")?;

    // accept connections and process them serially
    for stream in listener.incoming() {
        handle_client(&mut stream?);
    }
    Ok(())
}
