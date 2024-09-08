//! An end-to-end example of using the SP1 SDK to generate a proof of a program that can be executed
//! or have a core proof generated.
//!
//! You can run this script using the following command:
//! ```shell
//! RUST_LOG=info cargo run --release -- --execute
//! ```
//! or
//! ```shell
//! RUST_LOG=info cargo run --release -- --prove
//! ```

use std::{fs::File, time::Instant};

use clap::Parser;
// use evm_lib::PublicValuesStruct;
use sp1_sdk::{ProverClient, SP1Stdin};

/// The ELF (executable and linkable format) file for the Succinct RISC-V zkVM.
pub const STATELESS_VERIFIER_ELF: &[u8] =
    include_bytes!("../../client/elf/riscv32im-succinct-zkvm-elf");

/// The arguments for the command.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(long)]
    prove: bool,
}

use morph_executor::verify;
use sbv_primitives::{types::BlockTrace, Block, B256};

fn load_trace(file_path: &str) -> Vec<Vec<BlockTrace>> {
    use std::io::BufReader;

    let file = File::open(file_path).unwrap();
    let reader = BufReader::new(file);

    let chunk_traces: Vec<Vec<BlockTrace>> = serde_json::from_reader(reader).unwrap();

    chunk_traces
}

#[tokio::main]
async fn main() {
    // Setup the logger.
    sp1_sdk::utils::setup_logger();

    // Parse the command line arguments.
    let args = Args::parse();

    // Setup the prover client.
    let client = ProverClient::new();

    let traces: Vec<Vec<BlockTrace>> = load_trace("../../testdata/dev_tx_s.json");
    let trace_struct = &traces[0][0];
    println!("traces post state_root: {:?}", trace_struct.root_after());

    let trace = serde_json::to_string(trace_struct).unwrap();

    // Setup the inputs.
    let mut stdin = SP1Stdin::new();

    stdin.write(&trace);

    // Execute the program in sp1-vm
    let (mut public_values, execution_report) =
        client.execute(STATELESS_VERIFIER_ELF, stdin.clone()).run().unwrap();
    println!("Program executed successfully.");

    let pi_hash = public_values.read::<B256>();
    println!("pi_hash generated with sp1-vm execution: {}", hex::encode(pi_hash.as_slice()));

    // Execute the program in native
    let expected_hash = verify(trace_struct).unwrap_or_default();
    println!("pi_hash generated with native execution: {}", hex::encode(expected_hash.as_slice()));

    assert_eq!(pi_hash, expected_hash);
    println!("Values are correct!");

    // Record the number of cycles executed.
    println!("Number of cycles: {}", execution_report.total_instruction_count());
    if args.prove {
        let start = Instant::now();

        // Setup the program for proving.
        let (pk, vk) = client.setup(STATELESS_VERIFIER_ELF);

        // Generate the proof
        let proof = client.prove(&pk, stdin).run().expect("failed to generate proof");

        let duration_mins = start.elapsed().as_secs() / 60;
        println!("Successfully generated proof!, time use: {:?} minutes", duration_mins);

        // Verify the proof.
        client.verify(&proof, &vk).expect("failed to verify proof");
        println!("Successfully verified proof!");
    }
}

#[test]
fn test_prove() {
    let dev_elf: &[u8] = include_bytes!("../../client-dev/elf/riscv32im-succinct-zkvm-elf");

    // Setup the prover client.
    let client = ProverClient::new();

    // Setup the inputs.
    let mut stdin = SP1Stdin::new();

    let data = vec![1, 2];
    stdin.write(&data);

    // Execute the program in sp1-vm
    let (public_values, execution_report) = client.execute(dev_elf, stdin.clone()).run().unwrap();
    println!("Program executed successfully.");
    // Record the number of cycles executed.
    println!("Number of cycles: {}", execution_report.total_instruction_count());

    let rt_data = public_values.as_slice();
    println!("pi_hash generated with sp1-vm execution: {}", hex::encode(rt_data));

    let start = Instant::now();

    // Setup the program for proving.
    let (pk, vk) = client.setup(dev_elf);

    // Generate the proof
    let proof = client.prove(&pk, stdin).run().expect("failed to generate proof");

    let duration_secs = start.elapsed().as_secs();
    println!("Successfully generated proof!, time use: {:?} secs", duration_secs);

    // Verify the proof.
    client.verify(&proof, &vk).expect("failed to verify proof");
    println!("Successfully verified proof!");
}
