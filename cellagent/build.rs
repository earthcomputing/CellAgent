#[cfg(feature = "cell")]
use std::env::var;

fn main() {
    #[cfg(feature = "cell")]
    let cell_agent_dir = var("CELL_AGENT_DIR").expect("Must set CELL_AGENT_DIR environment variable");
    println!(r"cargo:rustc-link-search={}/ecnl", cell_agent_dir);
    println!(r"cargo:rustc-link-search={}/../../bjackson-ecnl/lib", cell_agent_dir);
}
