/*---------------------------------------------------------------------------------------------
 *  Copyright © 2016-present Earth Computing Corporation. All rights reserved.
 *  Licensed under the MIT License. See LICENSE.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/
#[cfg(feature = "cell")]
use std::env::var;

fn main() {
    println!("cargo:rerun-if-changed=/Users/alan/Documents/Eclipse/multicell/cellagent/src");
    #[cfg(feature = "cell")]
    let cell_agent_dir = var("CELL_AGENT_DIR").expect("Must set CELL_AGENT_DIR environment variable");
    #[cfg(feature = "cell")]
    println!(r"cargo:rustc-link-search={}/ecnl", cell_agent_dir);
    #[cfg(feature = "cell")]
    println!(r"cargo:rustc-link-search={}/../../driver/ecnl/lib", cell_agent_dir);
}
