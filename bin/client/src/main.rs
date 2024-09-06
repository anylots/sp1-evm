#![no_main]
sp1_zkvm::entrypoint!(main);

// use eth_types::l2_types::BlockTrace;
use stateless_block_verifier::{block_trace::BlockTrace, verify};

pub fn main() {
    let x = sp1_zkvm::io::read::<String>();
    let trace: BlockTrace = serde_json::from_str(&x).unwrap();
    let pi_hash = verify(&trace).unwrap();

    // Commit to the public values of the program. The final proof will have a commitment to all the
    // bytes that were committed to.
    sp1_zkvm::io::commit(&pi_hash);
}
