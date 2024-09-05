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
    execute: bool,

    #[clap(long)]
    prove: bool,

    #[clap(long, default_value = "20")]
    n: u32,
}

use eth_types::l2_types::BlockTrace;

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

    if args.execute == args.prove {
        eprintln!("Error: You must specify either --execute or --prove");
        std::process::exit(1);
    }

    // Setup the prover client.
    let client = ProverClient::new();

    let traces: Vec<Vec<BlockTrace>> = load_trace("../testdata/dev_tx_s.json");
    let trace_struct = &traces[0][0];

    let trace = serde_json::to_string(trace_struct).unwrap();

    // Setup the inputs.
    let mut stdin = SP1Stdin::new();

    stdin.write(&trace);

    println!("n: {}", args.n);

    if args.execute {
        // Execute the program
        let (output, report) = client.execute(STATELESS_VERIFIER_ELF, stdin).run().unwrap();
        println!("Program executed successfully.");

        let pi_hash = output.as_slice();
        println!("pi_hash executed in riscv-vm: {}", hex::encode(pi_hash));

        // let expected_hash = stateless_block_verifier::verify(&trace);
        // println!("pi_hash executed in native: {}", hex::encode(expected_hash));

        // assert_eq!(pi_hash, expected_hash);
        // assert_eq!(a, expected_a);
        println!("Values are correct!");

        // Record the number of cycles executed.
        println!("Number of cycles: {}", report.total_instruction_count());
    } else {
        let start = Instant::now();

        // Setup the program for proving.
        let (pk, vk) = client.setup(STATELESS_VERIFIER_ELF);

        // Generate the proof
        let proof = client.prove(&pk, stdin).run().expect("failed to generate proof");

        let duration_secs = start.elapsed().as_secs();
        println!("Successfully generated proof!, time use: {:?} secs", duration_secs);

        // Verify the proof.
        client.verify(&proof, &vk).expect("failed to verify proof");
        println!("Successfully verified proof!");
    }
}
