struct RoutingTable {
	entries: Vec<RoutingTableEntry>,
}

struct RoutingTableEntry {
	inuse: bool,
	tree_hash: String,
	mask: u16,
	indices: [usize; MAX_ENTRIES]
}