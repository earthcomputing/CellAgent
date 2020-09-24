Assuming this file is in the userspace/cellagent directory of CellAgent repository.

An environment variable CELL_AGENT_DIR must be set.
An environment variable SERVER_URL must be set if you want to send trace records to the web server.

This requires that the e1000e and ecnl driver kernel modules be loaded.

There are three executables in multicell/userspace/cellagent.
To execute:
$ cd userspace/cellagent
userspace/cellagent$ cargo run --bin simulator --features="simulator" -- config_file_name
userspace/cellagent$ cargo run --bin external_echoDemo --features="noc"

The third is cell.
Manual build process:
Make the driver interface code in C:
$ cd ecnl
userspace/cellagent/ecnl$ make
$ cd ..
userspace/cellagent$ cargo build --release --bin cell --features="cell"

Then, to execute:
userspace/cellagent$ sudo target/release/cell

Configuration files are read from userspace/cellagent/configs.

If you want to display the simulator output in a browser,
$ cd ../actix_server
userspace/actix_server$ cargo run
and when starting simulator:
userspace/cellagent$ cargo run --bin simulator --features="webserver simulator"  -- config_file_name

You MUST start the web server before the simulator.

Point a browser at SERVER_URL.

The server can replay from a trace file, two of which are in multicell/actix_server.

Tests are in src/test.rs
$ cargo test --features="simulator"
