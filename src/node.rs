pub struct Node {
    pub owner: Option<String>,
    pub sender: ws::Sender
}

impl Node {
    pub fn new(sender: ws::Sender) -> Node {
        Node {
            owner: None,
            sender: sender
        }
    }
}