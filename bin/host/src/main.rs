use morph_prover::prove;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    prove("../../testdata/mainnet_batch_traces.json");
}
