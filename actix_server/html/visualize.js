const x0 = 50;
const y0 = 50;
const scale = 50;
let cells = {};
let canvas;
window.onload = function() {
    canvas = document.getElementById("viz-canvas");
}
function visualize() {
    canvas.innerHTML = "";
    const Http = new XMLHttpRequest();
    const url = 'http://127.0.0.1:8088/';
    Http.open("GET", url + "geometry");
    Http.send();
    Http.onreadystatechange = (e) => {
        if ( Http.readyState == 4 && Http.status == 200 ) {
            setup_geometry(Http.responseText);
            Http.open("GET", url + "topology");
            Http.send();
            Http.onreadystatechange = (e) => {
                if ( Http.readyState == 4 && Http.status == 200 ) {
                    setup_topology(Http.responseText);
                    draw();
                    Http.open("GET", url + "black_tree");
                    Http.send();
                    Http.onreadystatechange = (e) => {
                        if ( Http.readyState == 4 && Http.status == 200 ) {
                            setup_trees(Http.responseText);
                        }
                    }
                }
            }
        }
    }
}
function setup_geometry(geometry_text) {
    let geometry = JSON.parse(geometry_text);
    let rowcol = geometry.geometry.rowcol;
    if ( Object.keys(rowcol).length == 0 ) { alert("Nothing to show.  Run simulator, and try again."); }
    for (cell in rowcol) {
        cells[cell] = rowcol[cell];
        cells[cell].neighbors = {};
        cells[cell].trees = {};
    }
}
function setup_topology(topology_text) {
    let topology = JSON.parse(topology_text);
    let appcells = topology.appcells;
    for (cellID in appcells) {
        let cellNeighbors = appcells[cellID].neighbors;
        cells[cellID].neighbors = cellNeighbors.neighbors;
        for (neighborIndex in cellNeighbors.neighbors) {
            let neighbor = cellNeighbors.neighbors[neighborIndex];
            let neighborID = neighbor.cell_name;
            if ( cellID < neighborID ) {
                let neighborPort = neighbor.port;
                let id = make_link_id(cellID, neighborIndex, neighborID , neighborPort);
                create_line_at(id, cellID, neighborID);
            }
        }
    }
    for (cellID in appcells) {
        create_node_at(cellID);
    }
}
function setup_trees(trees_text) {
    let trees = JSON.parse(trees_text);
    let appcells = trees.appcells;
    for (cellID in appcells) {
        let cellTrees = appcells[cellID].trees;
        cells[cellID].trees = cellTrees.trees;
    }
}
function make_link_id(cellID1, index1, cellID2, index2) {
    if ( cellID1 < cellID2 ) {
        return cellID1 + ":P" + index1 + "-" + cellID2 + ":P" + index2;
    } else {
        return cellID2 + ":P" + index2 + "-" + cellID1 + ":P" + index1;
    }
}
function create_line_at(id, cellID1, cellID2) {
    let line = document.createElement("line");
    line.setAttribute("id", id);
    line.setAttribute("class", "link");
    line.setAttribute("y1", x0 + scale*cells[cellID1].row);
    line.setAttribute("x1", y0 + scale*cells[cellID1].col);
    line.setAttribute("y2", x0 + scale*cells[cellID2].row);
    line.setAttribute("x2", y0 + scale*cells[cellID2].col);
    line.setAttribute("onclick", "link_click(evt)");
    line.setAttribute("ondblclick", "link_dblclick(evt)")
    let tooltipID = addTooltip(id);
    line.setAttribute("onmouseover", "showTooltip(" + tooltipID + ")");
    line.setAttribute("onmouseleave","hideTooltip(" + tooltipID + ")")
    canvas.appendChild(line);
    return line;
}
function create_node_at(id) {
    let x = cells[id].col;
    let y = cells[id].row;
    let circle = document.createElement("circle");
    circle.setAttribute("id", id);
    if (cells[id].is_border) {
        circle.setAttribute("class", "nodeborder");
    } else {
        circle.setAttribute("class", "node");
    }
    circle.setAttribute("cx", x0 + x*scale);
    circle.setAttribute("cy", y0 + y*scale);
    circle.setAttribute("onclick", "cell_click(evt)");
    circle.setAttribute("ondblclick", "cell_dblclick(evt)");
    let tooltipID = addTooltip(id);
    circle.setAttribute("onmouseover", "showTooltip(" + tooltipID + ")");
    circle.setAttribute("onmouseleave","hideTooltip(" + tooltipID + ")")
    canvas.appendChild(circle);
    return circle;
}
function showTooltip(elem) {
    elem.style.display = "block";
}
function hideTooltip(elem) {
    elem.style.display = "none";
}
function draw() { canvas.innerHTML = canvas.innerHTML; }
function cell_click(evt) {
    let links = document.querySelectorAll(".linktree");
    for (link of links) { link.setAttribute("class", "link"); }
    let nodes = document.querySelectorAll(".noderoot");
    for (node of nodes) { node.setAttribute("class", "node"); }
    let bordernodes = document.querySelectorAll(".noderootborder");
    for (node of bordernodes) { node.setAttribute("class", "nodeborder"); }
    let c = evt.target.getAttribute("class");
    if ( c == "node") {
        evt.target.setAttribute("class", "noderoot");
    } else if ( c == "nodeborder" ) {
        evt.target.setAttribute("class", "noderootborder");
    }
    let my_id = evt.target.getAttribute("id");
    draw_tree(my_id, "Tree:" + my_id);
}
function draw_tree(my_id, tree_id) {
    let cell = cells[my_id];
    let trees = cell.trees;
    let tree = trees[tree_id].tree;
    let neighbors = cell.neighbors;
    for (my_port in tree) {
        let neighbor_cell_id = neighbors[my_port].cell_name;
        if ( tree[my_port] == "Parent") {
            let neighbor_port = neighbors[my_port].port;
            let link_id = make_link_id(my_id, my_port, neighbor_cell_id, neighbor_port);
            document.getElementById(link_id).setAttribute("class", "linktree");
        }
        if ( tree[my_port] == "Child" ) {
            draw_tree(neighbor_cell_id, tree_id);
        }
    }
}
function cell_dblclick(evt) {
    evt.target.setAttribute("class", "nodebroken");
}
function link_click(evt) {
    if (evt.target.getAttribute("class") == "link") {
        evt.target.setAttribute("class", "linktree");
    } else {
        evt.target.setAttribute("class", "link");
    }
}
function link_dblclick(evt) {
    evt.target.setAttribute("class", "linkbroken")
}
function addTooltip(id) {
    let tooltipID = "tooltip" + id.replace(/:/g, "").replace(/-/g, "");
    let element = document.getElementById(tooltipID);
    if (typeof(element) == "undefined" || element == null) {
        let tooltip = document.createElement("div");
        tooltip.id = tooltipID;
        tooltip.style.display = "none";
        tooltip.setAttribute("class", "tooltip");
        tooltip.innerHTML = id;
        let body = document.getElementById("body");
        body.appendChild(tooltip);
    }
    return tooltipID;
}