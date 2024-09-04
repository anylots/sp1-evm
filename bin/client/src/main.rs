#![no_main]
sp1_zkvm::entrypoint!(main);

use alloy_sol_types::SolType;
use eth_types::l2_types::BlockTrace;
use stateless_block_verifier::{verify, PublicValuesStruct};

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
