struct Traph {
	tree_id: TreeID,
	table_index: usize,
	port: Port,
	status: TraphStatus
	
}
enum TraphStatus {
	Parent,
	Child,
	Pruned
}