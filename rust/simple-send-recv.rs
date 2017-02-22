#![feature(asm)] // to remove

#[macro_use]
extern crate hadean;

use hadean::{Sender, Receiver, Connection, ChannelEndpoint, Channel};
use hadean::{spawn, pid};

use std::mem;
use std::u64;

fn adder(toadd: &u64, mut senders: Vec<Sender>, mut receivers: Vec<Receiver>, _connections: Vec<Connection>) {
	let (sender, receiver) = (&mut senders[0], &mut receivers[0]);
	loop {
		let mut input: u64 = unsafe { mem::uninitialized() };
		receiver.receive(&mut input);
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

	let channels = vec![Channel::new(
		ChannelEndpoint::Sibling(0),
		ChannelEndpoint::Pid(pid())
	), Channel::new(
		ChannelEndpoint::Pid(pid()),
		ChannelEndpoint::Sibling(0)
	)];

	let (mut senders, mut receivers) = spawn(processes, channels);
	let (sender, receiver) = (&mut senders[0], &mut receivers[0]);

	let recv_u64 = || {
		let mut v: u64 = unsafe { mem::uninitialized() };
		receiver.receive(&mut v);
		v
	};

	sender.send::<u64>(&1);
	assert!(recv_u64() == 1 + toadd);
	sender.send::<u64>(&5);
	assert!(recv_u64() == 5 + toadd);
	sender.send::<u64>(&99999);
	assert!(recv_u64() == 99999 + toadd);
	sender.send::<u64>(&u64::MAX); // terminate subprocess

	println!("All adding successful");
}
