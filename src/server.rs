use std::str;
use std::rc::Rc;
use std::cell::RefCell;
use std::thread::sleep;
use std::time::Duration;

use serde_json::Value;

use ws::{listen, Handler, Result, Sender, Request, Message, Handshake, CloseCode, Response};
use ws::util::TcpStream;

use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod, SslStream};
use ws;
use crate::node::Node;
use crate::network::Network;

struct Server {
    node: Rc<RefCell<Node>>,
    ssl: Option<Rc<SslAcceptor>>,
    network: Rc<RefCell<Network>>,
}



impl Handler for Server {
    fn on_open(&mut self, handshake: Handshake) -> Result<()> {
        // Get the aruments from a URL
        // i.e localhost:8000/?user=testuser

        // skip()ing everything before the first '?' allows us to run the
        // server behind a reverse proxy like nginx with minimal fuss
        let url_arguments = handshake.request.resource()
            .split(|c| c=='?'||c=='='||c=='&').skip(1);
        // Beeing greedy by not collecting pairs
        // Instead every even number (including 0) will be an identifier
        // and every odd number will be the assigned value
        let argument_vector: Vec<&str> = url_arguments.collect();

        if argument_vector.len() >= 2 && argument_vector[0] == "user" {
            let username: &str = argument_vector[1];
            self.network.borrow_mut().add_user(username, &self.node);
        } else {
            println!("New node didn't provide a username");
        }

        println!("Network expanded to {:?} connected nodes", self.network.borrow().size());
        Ok(())
    }

    fn upgrade_ssl_server(&mut self, sock: TcpStream) -> ws::Result<SslStream<TcpStream>> {
        println!("Server node upgraded");
        // TODO  This is weird, but the sleep is needed...
        sleep(Duration::from_millis(200));
        self.ssl.as_mut().unwrap().accept(sock).map_err(From::from)
    }

    fn on_message(&mut self, msg: Message) -> Result<()> {
        let text_message: &str = msg.as_text()?;
        let json_message: Value =
            serde_json::from_str(text_message).unwrap_or(Value::default());

        // !!! WARNING !!!
        // The word "protocol" match is protcol specific.
        // Thus a client should make sure to send a viable protocol
        let protocol = match json_message["protocol"].as_str() {
            Some(desired_protocol) => { Some(desired_protocol) },
            _ => { None }
        };


        // The words below are protcol specific.
        // Thus a client should make sure to use a viable protocol
        let ret = match protocol {
            Some("one-to-all") => {
                self.node.borrow().sender.broadcast(text_message)
            },
            Some("one-to-self") => {
                self.node.borrow().sender.send(text_message)
            },
            Some("one-to-one") => {
                match json_message["endpoint"].as_str() {
                    Some(endpoint) => {
                        let network = self.network.borrow();
                        let endpoint_node = network.nodemap.borrow().get(endpoint)
                            .and_then(|node| node.upgrade());

                        match endpoint_node {
                            Some(node) => { node.borrow().sender.send(text_message) }
                            _ => {self.node.borrow().sender
                                .send(format!("Could not find a node with the name {}", endpoint))}
                        }
                    }
                    _ => {
                        self.node.borrow().sender.send(
                            "No field 'endpoint' provided"
                        )
                    }
                }

            }
            _ => {
                self.node.borrow().sender.send(
                    "Invalid protocol, valid protocols include:
                            'one-to-one'
                            'one-to-self'
                            'one-to-all'"
                )
            }
        };

        return ret
    }

    fn on_close(&mut self, code: CloseCode, reason: &str) {
        // Remove the node from the network
        if let Some(owner) = &self.node.borrow().owner {
            match code {
                CloseCode::Normal =>
                    println!("{:?} is done with the connection.", owner),
                CloseCode::Away =>
                    println!("{:?} left the site.", owner),
                CloseCode::Abnormal =>
                    println!("Closing handshake for {:?} failed!", owner),
                _ =>
                    println!("{:?} encountered an error: {:?}", owner, reason),
            };

            self.network.borrow_mut().remove(owner)
        }

        println!("Network shrinked to {:?} connected nodes\n", self.network.borrow().size());
    }

    fn on_error(&mut self, err: ws::Error) {
        println!("The server encountered an error: {:?}", err);
    }
}
// This can be read from a file
static INDEX_HTML: &'static [u8] = br#"
<!DOCTYPE html>
<html>
	<head>
		<meta charset="utf-8">
	</head>
	<body>
      <pre id="messages"></pre>
			<form id="form">
				<input type="text" id="msg">
				<input type="submit" value="Send">
			</form>
      <script>
        var socket = new WebSocket("ws://" + window.location.host + "/ws");
        socket.onmessage = function (event) {
          var messages = document.getElementById("messages");
          messages.append(event.data + "\n");
        };
        var form = document.getElementById("form");
        form.addEventListener('submit', function (event) {
          event.preventDefault();
          var input = document.getElementById("msg");
          socket.send(input.value);
          input.value = "";
        });
		</script>
	</body>
</html>
    "#;

struct Server2 {
    out: Sender,
}
impl Handler for Server2 {
    //
    fn on_request(&mut self, req: &Request) -> Result<(Response)> {
        // Using multiple handlers is better (see router example)
        match req.resource() {
            // The default trait implementation
            "/ws" => Response::from_request(req),

            // Create a custom response
            "/" => Ok(Response::new(200, "OK", INDEX_HTML.to_vec())),

            _ => Ok(Response::new(404, "Not Found", b"404 - Not Found".to_vec())),
        }
    }

    // Handle messages received in the websocket (in this case, only on /ws)
    fn on_message(&mut self, msg: Message) -> Result<()> {
        // Broadcast to all connections
        self.out.broadcast(msg)
    }
}

pub fn run(port: u16, use_ssl: bool, ssl_cert_path: Option<String>, ssl_cert_key_path: Option<String>) {
    let network = Rc::new(RefCell::new(Network::default()));

    let prefix = if use_ssl {"wss://"} else {"ws://"};

    let address = format!("{}0.0.0.0:{}", "", port);
    println!("Using ssl {} address {}", use_ssl, address);

    //listen(address, |out| Server2 { out }).unwrap();
    
    ws::Builder::new()
        .with_settings(ws::Settings {
            encrypt_server: use_ssl,
            ..ws::Settings::default()
        })
    .build(|sender: ws::Sender| {
        let acceptor = if use_ssl {Some(Rc::new({
            println!("Building acceptor");
            let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
            builder.set_private_key_file(ssl_cert_key_path.as_ref().unwrap(), SslFiletype::PEM).unwrap();
            builder.set_certificate_chain_file(ssl_cert_path.as_ref().unwrap()).unwrap();
            builder.build()
        }))} else {None};
        println!("Building server");
        let node = Node::new(sender);
        Server {
            node: Rc::new(RefCell::new(node)),
            ssl: acceptor,
            network: network.clone()
        }
    })

    .unwrap().listen(address).unwrap();
    

    println!("done websocket");
}
