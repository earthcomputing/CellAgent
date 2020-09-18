#!/bin/sh

# ./filter_traces.sh full_trace filtered_trace

# where full_trace is the name of the trace file produced by the emulator,
# and filtered_trace is the name of the file to contain the filtered records.

echo "Filtering trace file from $1 to $2"
egrep 'ca_process_discoverd_msg|border_cell_start|interior_cell_start|ca_process_stack_treed_msg|ca_process_hello_msg' "$1" > "$2"
