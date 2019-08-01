#[cfg(feature = "cell")]
use std::env::var;

fn main() {
    #[cfg(feature = "cell")]
    let cell_agent_dir = var("CELL_AGENT_DIR").unwrap_or("not defined".to_owned());
    #[cfg(feature = "cell")]
    println!("Cellagent directory {}", cell_agent_dir)
}
