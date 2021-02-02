use openssl::ssl::{SslMethod, SslAcceptor, SslStream, SslFiletype};
use std::net::{TcpListener, TcpStream, Shutdown};
use std::io::{Read, Write};
use std::sync::Arc;
use std::thread;

fn handle_client(mut stream: SslStream<TcpStream>) {
    let mut data = [0 as u8; 50];
    while match stream.read(&mut data) {
        Ok(size) => {
            stream.write(&data[0..size]).unwrap();
            true
        },
        Err(_) => {
            println!("Error with connection");
            //stream.shutdown(Shutdown::Both).unwrap();
            //todo handle & disconnect
            false
        }
    } {}
}

fn main() {
    let mut acceptor = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
    acceptor.set_private_key_file("key.pem", SslFiletype::PEM).unwrap();
    acceptor.set_certificate_chain_file("cert.pem").unwrap();
    acceptor.check_private_key().unwrap();
    let acceptor = Arc::new(acceptor.build());
    
    let listener = TcpListener::bind("0.0.0.0:42069").unwrap(); //todo handle error
    println!("Server started on port 42069");
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                println!("New connection: {}", stream.peer_addr().unwrap());
                let acceptor = acceptor.clone();
                thread::spawn(move|| {
                    let stream = acceptor.accept(stream).unwrap();
                    handle_client(stream)
                });
            }
            Err(e) => {
                println!("Error: {}", e);
            }
        }
    }
    drop(listener);
}
