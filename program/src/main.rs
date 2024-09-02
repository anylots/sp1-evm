//! A simple program that takes a number `n` as input, and writes the `n-1`th and `n`th fibonacci
//! number as an output.

// These two lines are necessary for the program to properly compile.
//
// Under the hood, we wrap your main function with some extra code so that it behaves properly
// inside the zkVM.
#![no_main]
sp1_zkvm::entrypoint!(main);

use alloy_sol_types::SolType;
use eth_types::l2_types::BlockTrace;
use evm_lib::{verify, PublicValuesStruct};

pub fn main() {
    let x = sp1_zkvm::io::read::<String>();

    let trace: BlockTrace = serde_json::from_str(&x).unwrap();

    verify(&trace).unwrap();

    // Encode the public values of the program.
    let bytes = PublicValuesStruct::abi_encode(&PublicValuesStruct { n: 20, a: 1, b: 2 });

    // Commit to the public values of the program. The final proof will have a commitment to all the
    // bytes that were committed to.
    sp1_zkvm::io::commit_slice(&bytes);
}
