
If you want to display the simulator output in a browser,
userspace$ cd actix_server
userspace/actix_server$ cargo run
and when starting simulator:
userspace/cellagent$ cargo run --bin simulator --features="webserver simulator"  -- config_file_name

You MUST start the web server before the simulator.

Point a browser at SERVER_URL.

The server can replay from a trace file, two of which are in multicell/actix_server.
