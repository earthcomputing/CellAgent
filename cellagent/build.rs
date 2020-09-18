#[cfg(feature = "cell")]
use std::env::var;

fn main() {
    println!("cargo:rerun-if-changed=/Users/alan/Documents/Eclipse/multicell/cellagent/src");
    #[cfg(feature = "cell")]
    let cell_agent_dir = var("CELL_AGENT_DIR").expect("Must set CELL_AGENT_DIR environment variable");
    #[cfg(feature = "cell")]
    println!(r"cargo:rustc-link-search={}/ecnl", cell_agent_dir);
    #[cfg(feature = "cell")]
    println!(r"cargo:rustc-link-search={}/../../bjackson-ecnl/lib", cell_agent_dir);
}
