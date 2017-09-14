#![feature(asm)]

#[macro_use]
extern crate hadean;
extern crate hadean_std;
extern crate rand;
extern crate serde;
#[macro_use]
extern crate serde_derive;

use std::{cmp, env, mem};

use hadean::{RawSender, RawReceiver, Sender, Receiver, Connection, ChannelEndpoint, Channel, ProcessTransfer};
use hadean::{pid, spawn};
use hadean_std::pinnedvec::PinnedVec;
use rand::{SeedableRng, Rng};
use rand::distributions::IndependentSample;

use self::Msg::*;

pub trait Phenotype {
	fn init(&mut self);
	fn utility(&self) -> f64;
	fn crossover(&mut self, other: &mut Self);
	fn mutate(&mut self);
}

fn select(rng: &mut rand::XorShiftRng, utilities: &[f64]) -> usize {
	let rnd = rng.next_f64(); // [0.0,1.0)
	let mut i = 1;
	while utilities[i] < rnd && i < utilities.len() {
		i += 1;
	}
	i-1
}

#[derive(Clone,Copy)]
#[derive(Serialize, Deserialize)]
struct WorkerArgs {
	index: usize,
	num_workers: usize,
	phenotype_goal: f64,
	population_count: usize,
	generations_max: usize,
	mutate_count: usize,
	crossover_count: usize,
	migrate_count: usize
}
#[derive(Serialize, Deserialize)]
enum Msg<T> {
	Done(bool),
	Phenotype(T),
	Best(f64),
}
impl<T: Phenotype + ProcessTransfer> Msg<T> {
	fn unwrap_done(self) -> bool { if let Done(b) = self { b } else { panic!("not a done msg (bool)") } }
	fn unwrap_pheno(self) -> T { if let Phenotype(p) = self { p } else { panic!("not a phenotype") } }
	fn unwrap_best(self) -> f64 { if let Best(f) = self { f } else { panic!("not a best msg (f64)") } }
}
fn worker<T: Copy + Phenotype + ProcessTransfer>(args: &WorkerArgs, senders: Vec<RawSender>, receivers: Vec<RawReceiver>, _connections: Vec<Connection>) {
	let mut senders = senders.into_iter();
	let mut receivers = receivers.into_iter();

	let mut rng = rand::XorShiftRng::from_seed([1,2,3,4]);
	let mut population: PinnedVec<T> = PinnedVec::with_capacity(args.population_count);
	unsafe { population.set_len(args.population_count) };

	println!("initialising {:?}", pid());
	for i in 0..args.population_count {
		population[i].init();
	}

	let from_left: Receiver<Msg<T>> = receivers.next().unwrap().downcast();
	let to_right: Sender<Msg<T>> = senders.next().unwrap().downcast();

	let mut best: f64;
	let mut winner = population[0];

	let mut generation = 0;
	loop {
		let mut utilities = PinnedVec::with_capacity(args.population_count);
		best = 0.0;
		for i in 0..args.population_count {
			utilities.push(population[i].utility()).unwrap();
			if utilities[i] > best {
				best = utilities[i];
				winner = population[i];
			}
		}

		let mut done = best >= args.phenotype_goal;

		println!("{:?}: gen {}: {}/{}", pid(), generation, best, args.phenotype_goal);

		if args.index == 0 {
			to_right.send(&Done(done));
			done = from_left.recv().unwrap_done();
			to_right.send(&Done(done));
			done = from_left.recv().unwrap_done();
		} else {
			let done2 = from_left.recv().unwrap_done();
			done |= done2;
			to_right.send(&Done(done));
			done = from_left.recv().unwrap_done();
			to_right.send(&Done(done));
		}

		if done || generation == args.generations_max {
			break
		}

		if args.index == 0 {
			let mut migrating = PinnedVec::with_capacity(args.migrate_count);
			for _ in 0..args.migrate_count {
				let k = rng.gen_range(0, args.population_count);
				migrating.push(k).unwrap();
				to_right.send(&Phenotype(population[k]));
			}
			for j in 0..args.migrate_count {
				let k = migrating[j];
				population[k] = from_left.recv().unwrap_pheno();
			}
		} else {
			let mut migrating_phenotypes = PinnedVec::with_capacity(args.migrate_count);
			for _ in 0..args.migrate_count {
				let k = rng.gen_range(0, args.population_count);
				migrating_phenotypes.push(population[k]).unwrap();
				population[k] = from_left.recv().unwrap_pheno();
			}
			for j in 0..args.migrate_count {
				to_right.send(&Phenotype(migrating_phenotypes[j]));
			}
		}

		generation += 1;

		let mut utility_sum: f64 = 0.0;
		for i in 0..args.population_count {
			utility_sum += utilities[i];
		}
		for i in 0..args.population_count {
			utilities[i] /= utility_sum;
		}
		for i in 1..args.population_count {
			utilities[i] += utilities[i-1];
		}

		for _ in 0..args.crossover_count {
			let j = select(&mut rng, &utilities);
			let k = select(&mut rng, &utilities);
			if j == k {
				continue;
			}
			let (off1, off2) = (cmp::min(j, k), cmp::max(j, k));
			let (pop1, pop2) = population.split_at_mut(off2);
			pop1[off1].crossover(&mut pop2[0]);
		}

		for _ in 0..args.mutate_count {
			let j = rng.gen_range(0, args.population_count);
			population[j].mutate();
		}
	}

	println!("{:?}: done: {}/{}", pid(), best, args.phenotype_goal);

	if args.index != 0 {
		let left_best = from_left.recv().unwrap_best();
		let left_winner = from_left.recv().unwrap_pheno();
		if left_best > best {
			best = left_best;
			winner = left_winner;
		}
	}
	if args.index != args.num_workers-1 {
		to_right.send(&Best(best));
		to_right.send(&Phenotype(winner));
	}
	if args.index == args.num_workers-1 {
		let parent_channel: Sender<T> = senders.next().unwrap().downcast();
		parent_channel.send(&winner);
	}
}

pub fn run<T: Copy + Phenotype + ProcessTransfer>(num_workers: usize, phenotype_goal: f64, population_count: usize, generations_max: usize, mutate_count: usize, crossover_count: usize, migrate_count: usize) -> T {
	println!("running genetic algorithm across {} subpopulations", num_workers);

	let mut processes = Vec::new();
	for i in 0..num_workers {
		processes.push(mkprocess!(worker::<T>, WorkerArgs {
			index: i,
			num_workers: num_workers,
			phenotype_goal: phenotype_goal,
			population_count: population_count,
			generations_max: generations_max,
			mutate_count: mutate_count,
			crossover_count: crossover_count,
			migrate_count: migrate_count,
		}));
	}

	let mut channels = Vec::new();
	for i in 0..num_workers {
		channels.push(Channel::new::<Msg<T>>(
			ChannelEndpoint::Sibling(i),
			ChannelEndpoint::Sibling((i + 1) % num_workers)
		));
	}
	channels.push(Channel::new::<T>(
		ChannelEndpoint::Sibling(num_workers-1),
		ChannelEndpoint::Pid(pid())
	));

	println!("spawn");
	let (_senders, receivers) = spawn(processes, channels);
	println!("/spawn");

	let receiver: Receiver<T> = receivers.into_iter().next().unwrap().downcast();
	let result = receiver.recv();
	result
}

fn do_ga(num_workers: usize) {
	// TODO: expand to 128 when serde supports it: https://github.com/serde-rs/serde/issues/631
	#[derive(Copy)]
	#[derive(Serialize, Deserialize)]
	struct MyPhenotype([u8; 32]);
	impl Clone for MyPhenotype {
		fn clone(&self) -> Self { MyPhenotype(self.0) }
	}
	impl Phenotype for MyPhenotype {
		fn init(&mut self) {
			rand::thread_rng().fill_bytes(&mut self.0);
		}
		fn utility(&self) -> f64 {
			self.0.iter().fold(0.0f64, |acc, &x| acc + x as f64)
		}
		fn crossover(&mut self, other: &mut Self) {
			let tmp1 = *self;
			let tmp2 = *other;
			let u1 = self.utility();
			let u2 = other.utility();
			let mut rng = rand::thread_rng();
			let range = rand::distributions::range::Range::new(0, self.0.len() - 1);
			let split = 1 + range.ind_sample(&mut rng);
			for (v1, v2) in self.0.iter_mut().zip(other.0.iter_mut()).skip(split) { mem::swap(v1, v2) }
			if self.utility() < u1 { *self = tmp1 }
			if other.utility() < u2 { *self = tmp2 }
		}
		fn mutate(&mut self) {
			let u1 = self.utility();
			let mut rng = rand::thread_rng();
			let range = rand::distributions::range::Range::new(0, self.0.len());
			let pos = range.ind_sample(&mut rng);
			let prev = self.0[range.ind_sample(&mut rng)];
			self.0[range.ind_sample(&mut rng)] = rng.gen();
			let u2 = self.utility();
			if u1 > u2 { self.0[pos] = prev; }
		}
	}
	let phenotype_goal = (128usize*255) as f64 * 0.95;
	let population_count = 1000;
	let generations_max = 10;
	let mutate_count = population_count/3;
	let crossover_count = population_count/2;
	let migrate_count = 50;
	let result = run::<MyPhenotype>(num_workers, phenotype_goal, population_count, generations_max, mutate_count, crossover_count, migrate_count);
	print!("result: ");
	for byte in result.0.iter() {
		print!("{:02x}", byte);
	}
	print!("\n");
	println!("done ga");
}

fn main() {
	let num_workers: usize = env::args().skip(1)
		.next().expect("Expected an arg")
		.parse().expect("Failed to parse arg as a num");
	do_ga(num_workers)
}
