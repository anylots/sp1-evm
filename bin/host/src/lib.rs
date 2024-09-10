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

pub fn prove_for_queue() {}

pub fn prove(trace_path: &str) {
    // Setup the logger.
    sp1_sdk::utils::setup_logger();

    // Parse the command line arguments.
    let args = Args::parse();

    // Setup the prover client.

    let mut traces: Vec<Vec<BlockTrace>> = load_trace(trace_path);
    let trace_struct: &mut BlockTrace = &mut traces[0][1];
    println!("traces' post_state_root: {:?}", trace_struct.root_after());
    println!(
        "traces' transactions.len(): {:?}",
        trace_struct.transactions.len()
    );
    let st = trace_struct.storage_trace.clone();
    println!(
        "traces' flatten_proofs last address: {:?}",
        st.proofs.unwrap().first().unwrap().0
    );

    trace_struct.flatten();
    println!(
        "traces' flatten_proofs last address: {:?}",
        trace_struct.flatten_proofs().last().unwrap().0
    );

    let trace_str = serde_json::to_string(trace_struct).unwrap();

    // Execute the program in native
    let expected_hash = verify(trace_struct).unwrap_or_default();
    println!(
        "pi_hash generated with native execution: {}",
        hex::encode(expected_hash.as_slice())
    );

    let client = ProverClient::new();
    // Setup the inputs.
    let mut stdin = SP1Stdin::new();
    stdin.write(&trace_str);

    // Execute the program in sp1-vm
    let (mut public_values, execution_report) = client
        .execute(STATELESS_VERIFIER_ELF, stdin.clone())
        .run()
        .unwrap();
    println!("Program executed successfully.");

    let pi_hash = public_values.read::<B256>();
    println!(
        "pi_hash generated with sp1-vm execution: {}",
        hex::encode(pi_hash.as_slice())
    );

    assert_eq!(pi_hash, expected_hash);
    println!("Values are correct!");

    // Record the number of cycles executed.
    println!(
        "Number of cycles: {}",
        execution_report.total_instruction_count()
    );
    if args.prove {
        let start = Instant::now();

        // Setup the program for proving.
        let (pk, vk) = client.setup(STATELESS_VERIFIER_ELF);

        // Generate the proof
        let proof = client
            .prove(&pk, stdin)
            .run()
            .expect("failed to generate proof");

        let duration_mins = start.elapsed().as_secs() / 60;
        println!(
            "Successfully generated proof!, time use: {:?} minutes",
            duration_mins
        );

        // Verify the proof.
        client.verify(&proof, &vk).expect("failed to verify proof");
        println!("Successfully verified proof!");
    }
}
