use std::str;
use std::rc::Rc;
use std::rc::Weak;
use std::cell::RefCell;
use std::collections::HashMap;

use crate::node::Node;

#[derive(Default)]
pub struct Network {
    pub nodemap: Rc<RefCell<HashMap<String, Weak<RefCell<Node>>>>>,
}

impl Network {
    pub fn add_user(&mut self, owner: &str, node: &std::rc::Rc<std::cell::RefCell<Node>>) {
        if !self.nodemap.borrow().contains_key(owner) {
            node.borrow_mut().owner = Some(owner.into());
            self.nodemap.borrow_mut().insert(owner.to_string(), Rc::downgrade(node));
            println!("Node {:?} connected to the network.", owner);
        } else {
            println!("{:?} tried to connect, but the username was taken", owner);
            node.borrow().sender.send("The username is taken").ok();
        }
    }

    pub fn remove(&mut self, owner: &str) {
        self.nodemap.borrow_mut().remove(owner);
    }

    pub fn size(&self) -> usize {
        self.nodemap.borrow().len()
    }
}