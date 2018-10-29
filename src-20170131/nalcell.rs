struct NalCell {
	id: CellID,
	ports: [Port; MAX_PHYSICAL_PORTS],
	cell_agent: CellAgent,
	vms: Vec<VirtualMachine>,
}