use std::sync::{Arc, Mutex};
use std::sync::mpsc::{self, channel};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::thread;

use websocket::{Server, Message, Sender};
use websocket::server::request::RequestUri;
use websocket::result::WebSocketError;

use toml;

use diecast::{self, Handle, Bind};
use diecast::util::handle::item;

// TODO
// this preferably shouldn't have to be in an arc mutex
// rethink the entire design, perhaps require a Clone
// bound instead of a Sync + Send bound
pub struct WebsocketPipe {
    ws_tx: Mutex<mpsc::Sender<Update>>,
}

pub fn pipe(ws_tx: mpsc::Sender<Update>) -> WebsocketPipe {
    WebsocketPipe {
        ws_tx: Mutex::new(ws_tx),
    }
}

impl Handle<Bind> for WebsocketPipe {
    fn handle(&self, bind: &mut Bind) -> diecast::Result {
        let ws_tx = {
            let sender = self.ws_tx.lock().unwrap();
            sender.clone()
        };

        for item in bind {
            if let Some(meta) = item.extensions.get::<item::Metadata>() {
                if !meta.lookup("push").and_then(toml::Value::as_bool).unwrap_or(true) {
                    continue;
                }
            }

            let uri = format!("/{}", item.route.reading().unwrap().display());

            ws_tx.send(Update {
                url: uri,
                body: item.body.clone(),
            }).unwrap();
        }

        Ok(())
    }
}

pub struct Update {
    url: String,
    body: String,
}

pub fn init() -> mpsc::Sender<Update> {
    let (tx, rx) = channel::<Update>();
    let channels: Arc<Mutex<HashMap<String, HashMap<SocketAddr, ::websocket::server::sender::Sender<_>>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let reader = channels.clone();
    let writer = channels.clone();

    thread::spawn(move || {
        for update in rx.iter() {
            let mut reader = reader.lock().unwrap();
            let mut prune = vec![];

            if let Some(channels) = reader.get_mut(&update.url) {
                for (addr, sender) in channels.iter_mut() {
                    match sender.send_message(Message::Text(update.body.clone())) {
                        Ok(()) => println!("sent new"),
                        // handle the case where the user disconnected
                        Err(WebSocketError::IoError(e)) => {
                            if let ::std::io::ErrorKind::BrokenPipe = e.kind() {
                                // TODO: need to remove the client if this occurs
                                println!("client from {} disconnected", addr);
                                prune.push(addr.clone());
                            } else {
                                panic!(e);
                            }
                        },
                        Err(e) => panic!(e),
                    }
                }

                for addr in prune {
                    println!("pruning {}", addr);
                    channels.remove(&addr).unwrap();
                }
            }
        }
    });

    thread::spawn(move || {
        let server = Server::bind("0.0.0.0:9160").unwrap();

        for (idx, connection) in server.enumerate() {
            let writer = writer.clone();

            thread::spawn(move || {
                let request = connection.unwrap().read_request().unwrap();

                let uri = match request.url {
                    RequestUri::AbsolutePath(ref path) => {
                        println!("request received for {}", path);
                        Some(path.clone())
                    },
                    _ => None,
                };

                let response = request.accept();
                let mut client = response.send().unwrap();

                let ip = client.get_mut_sender().get_mut().peer_addr().unwrap();
                println!("WS connection #{} from {}", idx + 1, ip);

                // TODO should monitor receiver to detect close events
                let (sender, _receiver) = client.split();

                let mut writer = writer.lock().unwrap();

                writer
                    .entry(uri.unwrap()).or_insert(HashMap::new())
                    .entry(ip).or_insert(sender);
            });
        }
    });

    tx
}


