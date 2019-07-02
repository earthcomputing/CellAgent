use std::env::var;

fn main() {
    let cell_agent_dir = var("CELL_AGENT_DIR").unwrap();
    println!(r"cargo:rustc-link-search={}/ecnl", cell_agent_dir);
}
