#[cfg(feature = "cell")]
use std::env::var;

fn main() {
    #[cfg(feature = "cell")]
    println!(r"cargo:rustc-link-search={}/ecnl", var("CELL_AGENT_DIR").expect("Must set CELL_AGENT_DIR environment variable"));
}
