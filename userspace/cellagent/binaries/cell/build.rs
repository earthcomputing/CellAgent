use std::env::var;

fn main() {
    let cell_agent_dir = var("CELL_AGENT_DIR").expect("Must set CELL_AGENT_DIR environment variable");
    println!(r"cargo:rustc-link-search={}/ecnl", cell_agent_dir);
}
