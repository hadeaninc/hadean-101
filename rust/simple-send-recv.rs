#![feature(asm)] // to remove

#[macro_use]
extern crate hadean;

use hadean::{RawSender, RawReceiver, Sender, Receiver, Connection, ChannelEndpoint, Channel};
use hadean::{spawn, pid};

use std::u64;

fn adder(toadd: &u64, senders: Vec<RawSender>, receivers: Vec<RawReceiver>, _connections: Vec<Connection>) {
	let mut senders = senders.into_iter();
	let mut receivers = receivers.into_iter();

	let receiver: Receiver<u64> = receivers.next().unwrap().downcast();
	let sender: Sender<u64> = senders.next().unwrap().downcast();

	loop {
		let input = receiver.recv();
		if input == u64::MAX {
			break
		}
		let output = input + toadd;
		sender.send(&output);
	}
}

fn main() {
	let toadd: u64 = 15;
	let processes = vec![mkprocess!(adder, toadd)];

	let channels = vec![Channel::new::<u64>(
		ChannelEndpoint::Sibling(0),
		ChannelEndpoint::Pid(pid())
	), Channel::new::<u64>(
		ChannelEndpoint::Pid(pid()),
		ChannelEndpoint::Sibling(0)
	)];

	let (senders, receivers) = spawn(processes, channels);
	let mut senders = senders.into_iter();
	let mut receivers = receivers.into_iter();

	let receiver: Receiver<u64> = receivers.next().unwrap().downcast();
	let sender: Sender<u64> = senders.next().unwrap().downcast();

	sender.send(&1);
	assert!(receiver.recv() == 1 + toadd);
	sender.send(&5);
	assert!(receiver.recv() == 5 + toadd);
	sender.send(&99999);
	assert!(receiver.recv() == 99999 + toadd);
	sender.send(&u64::MAX); // terminate subprocess

	println!("All adding successful");
}
