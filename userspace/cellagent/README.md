Assuming this file is in the userspace/cellagent directory of CellAgent repository.

An environment variable CELL_AGENT_DIR must be set.
An environment variable SERVER_URL must be set if you want to send trace records to the web server.

This requires that the e1000e and ecnl driver kernel modules be loaded.

Three executables may be generated.

Simulator simulates the driver operation of multiple cells and also performs higher-level processing on and between cells.  NOC is intended to operate indepenently of any particular cell.  Simulator and NOC are executed directly:
```
$ cd userspace/cellagent
userspace/cellagent$ cargo run --bin simulator --features="simulator" -- config_file_name
userspace/cellagent$ cargo run --bin external_echoDemo --features="noc"
```

The third is cell, which uses the driver.

## Manual build process
Make the driver interface code in C:
```
$ cd ecnl
userspace/cellagent/ecnl$ make
$ cd ..
userspace/cellagent$ cargo build --release --bin cell --features="cell"
```

Then, to execute:
```
userspace/cellagent$ sudo target/release/cell
```

Configuration files are read from userspace/cellagent/configs.

Tests are in src/test.rs
```
$ cargo test --features="simulator"
```