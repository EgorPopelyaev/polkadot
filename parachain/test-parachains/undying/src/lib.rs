// Copyright 2017-2020 Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

//! Basic parachain that adds a number as part of its state.

#![no_std]
#![cfg_attr(
	not(feature = "std"),
	feature(core_intrinsics, lang_items, core_panic_info, alloc_error_handler)
)]

use parity_scale_codec::{Decode, Encode};
use sp_std::vec::Vec;
use tiny_keccak::{Hasher as _, Keccak};

#[cfg(not(feature = "std"))]
mod wasm_validation;

#[cfg(not(feature = "std"))]
#[global_allocator]
static ALLOC: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

fn keccak256(input: &[u8]) -> [u8; 32] {
	let mut out = [0u8; 32];
	let mut keccak256 = Keccak::v256();
	keccak256.update(input);
	keccak256.finalize(&mut out);
	out
}

/// Wasm binary unwrapped. If built with `BUILD_DUMMY_WASM_BINARY`, the function panics.
#[cfg(feature = "std")]
pub fn wasm_binary_unwrap() -> &'static [u8] {
	WASM_BINARY.expect(
		"Development wasm binary is not available. Testing is only \
						supported with the flag disabled.",
	)
}

/// Head data for this parachain.
#[derive(Default, Clone, Hash, Eq, PartialEq, Encode, Decode, Debug)]
pub struct HeadData {
	/// Block number
	pub number: u64,
	/// parent block keccak256
	pub parent_hash: [u8; 32],
	/// hash of post-execution state.
	pub post_state: [u8; 32],
}

impl HeadData {
	pub fn hash(&self) -> [u8; 32] {
		keccak256(&self.encode())
	}
}

/// Block data for this parachain.
#[derive(Default, Clone, Encode, Decode, Debug)]
pub struct GraveyardState {
	/// The grave index of the last tombstone.
	pub index: u64,
	/// We use a matrix where each element represents a grave.
	/// The unsigned integer tracks the number of tombstones errected on 
	/// each trave.
	pub graveyard: Vec<u64>,
	// TODO: Add zombies. All of the graves produce zombies at a regular interval
	// defined in blocks. The number of zombies produced scales with the tombstones.
	// This would allow us to have a configurable and reproducible PVF execution time. 
	// However, PVF preparation time will likely rely on prebuild wasm binaries.
}

/// Block data for this parachain.
#[derive(Default, Clone, Encode, Decode, Debug)]
pub struct BlockData {
	/// The state
	pub state: GraveyardState,
	/// The number of tombstones to errect.
	pub tombstones: u64,
}

pub fn hash_state(state: &GraveyardState) -> [u8; 32] {
	keccak256(state.encode().as_slice())
}

/// Executes a graveyard transaction.
pub fn execute_transaction(mut block_data: BlockData) -> GraveyardState {
	for i in 0..block_data.tombstones {
		block_data.state.graveyard[(block_data.state.index + i) as usize] =
			block_data.state.graveyard[(block_data.state.index + i) as usize].wrapping_add(1);
	}
	block_data.state.index = ((block_data.state.index.saturating_add(block_data.tombstones))
		as usize % block_data.state.graveyard.len()) as u64;
	block_data.state
}

/// Start state mismatched with parent header's state hash.
#[derive(Debug)]
pub struct StateMismatch;

/// Execute a block body on top of given parent head, producing new parent head
/// and new state if valid.
pub fn execute(
	parent_hash: [u8; 32],
	parent_head: HeadData,
	block_data: BlockData,
) -> Result<(HeadData, GraveyardState), StateMismatch> {
	assert_eq!(parent_hash, parent_head.hash());

	if hash_state(&block_data.state) != parent_head.post_state {
		log::error!(
			"state has diff vs head: {:?} vs {:?}",
			hash_state(&block_data.state),
			parent_head.post_state,
		);
		return Err(StateMismatch)
	}

	let new_state = execute_transaction(block_data.clone());

	Ok((
		HeadData {
			number: parent_head.number + 1,
			parent_hash,
			post_state: hash_state(&new_state),
		},
		new_state,
	))
}
