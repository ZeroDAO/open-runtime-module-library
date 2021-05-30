#![cfg_attr(not(feature = "std"), no_std)]

#[doc(hidden)]
pub extern crate frame_benchmarking;
#[doc(hidden)]
pub extern crate sp_core;
#[doc(hidden)]
pub extern crate sp_std;

mod macros;

#[cfg(feature = "std")]
pub mod bench_runner;
#[cfg(feature = "std")]
pub mod build_wasm;
#[cfg(feature = "std")]
mod colorize;
#[cfg(feature = "std")]
pub mod handler;
#[cfg(feature = "std")]
mod redundant_meter;

use codec::{Decode, Encode};
use sp_std::{
	cmp::max,
	prelude::{Box, Vec},
};

#[derive(Encode, Decode, Default, Clone, PartialEq, Debug)]
pub struct BenchResult {
	pub method: Vec<u8>,
	pub elapses: Vec<u128>,
	pub reads: u32,
	pub repeat_reads: u32,
	pub writes: u32,
	pub repeat_writes: u32,
}

pub struct Bencher {
	pub results: Vec<BenchResult>,
	pub prepare: Box<dyn Fn()>,
	pub verify: Box<dyn Fn()>,
}

impl Default for Bencher {
	fn default() -> Self {
		Bencher {
			results: Vec::new(),
			prepare: Box::new(|| {}),
			verify: Box::new(|| {}),
		}
	}
}

impl Bencher {
	/// Reset prepare and verify block
	pub fn reset(&mut self) {
		self.prepare = Box::new(|| {});
		self.verify = Box::new(|| {});
	}

	/// Set prepare block
	pub fn set_prepare(&mut self, prepare: impl Fn() + 'static) -> &mut Self {
		self.prepare = Box::new(prepare);
		self
	}

	/// Set verify block
	pub fn set_verify(&mut self, verify: impl Fn() + 'static) -> &mut Self {
		self.verify = Box::new(verify);
		self
	}

	/// Run benchmark for block
	pub fn bench<F: Fn()>(&mut self, name: &str, block: F) {
		// Warm up the DB
		frame_benchmarking::benchmarking::commit_db();
		frame_benchmarking::benchmarking::wipe_db();

		let mut result = BenchResult {
			method: name.as_bytes().to_vec(),
			..Default::default()
		};

		for _ in 0..50 {
			// Execute prepare block
			(self.prepare)();

			frame_benchmarking::benchmarking::commit_db();
			frame_benchmarking::benchmarking::reset_read_write_count();

			let start_time = frame_benchmarking::benchmarking::current_time();
			// Execute bench block
			block();
			let end_time = frame_benchmarking::benchmarking::current_time();
			frame_benchmarking::benchmarking::commit_db();

			let (elapsed, reads, repeat_reads, writes, repeat_writes) =
				bencher::finalized_results(end_time - start_time);

			// Execute verify block
			(self.verify)();

			// Reset the DB
			frame_benchmarking::benchmarking::wipe_db();

			result.elapses.push(elapsed);

			result.reads = max(result.reads, reads);
			result.repeat_reads = max(result.repeat_reads, repeat_reads);
			result.writes = max(result.writes, writes);
			result.repeat_writes = max(result.repeat_writes, repeat_writes);
		}
		self.results.push(result);
	}
}

#[cfg(feature = "std")]
thread_local! {
	static REDUNDANT_METER: std::cell::RefCell<redundant_meter::RedundantMeter> = std::cell::RefCell::new(redundant_meter::RedundantMeter::default());
}

#[sp_runtime_interface::runtime_interface]
pub trait Bencher {
	fn entering_method() -> Vec<u8> {
		REDUNDANT_METER.with(|x| x.borrow_mut().entering_method())
	}

	fn leaving_method(identifier: Vec<u8>) {
		REDUNDANT_METER.with(|x| {
			x.borrow_mut().leaving_method(identifier);
		});
	}

	fn finalized_results(elapsed: u128) -> (u128, u32, u32, u32, u32) {
		let (reads, repeat_reads, writes, repeat_writes) = frame_benchmarking::benchmarking::read_write_count();

		let (redundant_elapsed, redundant_reads, redundant_repeat_reads, redundant_writes, redundant_repeat_writes) =
			REDUNDANT_METER.with(|x| x.borrow_mut().take_results());

		let elapsed = elapsed - redundant_elapsed;
		let reads = reads - redundant_reads;
		let repeat_reads = repeat_reads - redundant_repeat_reads;
		let writes = writes - redundant_writes;
		let repeat_writes = repeat_writes - redundant_repeat_writes;

		(elapsed, reads, repeat_reads, writes, repeat_writes)
	}
}
