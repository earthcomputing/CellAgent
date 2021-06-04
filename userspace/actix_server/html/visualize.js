/*---------------------------------------------------------------------------------------------
 *  Copyright © 2016-present Earth Computing Corporation. All rights reserved.
 *  Licensed under the MIT License. See LICENSE.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/
const x0 = 50;
const y0 = 50;
const scale = 50;
let cells = {};
let all_stacked_trees = [];
let canvas;
window.onload = function() {
    canvas = document.getElementById("viz-canvas");
    visualize();
}
function clear_dispay() {
    let buttons = document.querySelectorAll(".stackedtreebutton");
    if (buttons.length > 0 ) {
        for (button of buttons) {
            button.parentNode.removeChild(button);
        }
    }
    while (canvas.lastChild) {
        canvas.removeChild(canvas.lastChild);
    }
}
function visualize() {
    canvas.innerHTML = "";
    const Http = new XMLHttpRequest();
    clear_dispay();
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
                            setup_black_trees(Http.responseText);
                            Http.open("GET", url + "stack_treed");
                            Http.send();
                            Http.onreadystatechange = (e) => {
                                if ( Http.readyState == 4 && Http.status == 200 ) {
                                    setup_stacked_trees(Http.responseText);
                                }
                            }
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
    if ( Object.keys(rowcol).length == 0 ) {
        alert("Nothing to show.  Run simulator, and try again.");
        return 1;
    }
    for (cell in rowcol) {
        cells[cell] = rowcol[cell];
        cells[cell].neighbors = {};
        cells[cell].black_trees = {};
        cells[cell].stacked_trees = {}
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
            let cellIDint = parseInt(cellID.split(":")[1], 10);
            let neighborIDint = parseInt(neighborID.split(":")[1], 10);
            if ( cellIDint < neighborIDint ) {
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
function setup_black_trees(black_trees_text) {
    let trees = JSON.parse(black_trees_text);
    let appcells = trees.appcells;
    for (cellID in appcells) {
        let cellTrees = appcells[cellID].black_trees;
        cells[cellID].black_trees = cellTrees.trees;
    }
}
function setup_stacked_trees(stacked_trees_text) {
    let trees = JSON.parse(stacked_trees_text);
    let appcells = trees.appcells;
    let buttons = document.getElementById("buttons");
    for (cellID in appcells) {
        let cell = cells[cellID];
        cell.stacked_trees = appcells[cellID].stacked_trees;
        let stacked_trees = cell.stacked_trees;
        for (stacked_tree in stacked_trees.trees) {
            if (!document.getElementById(stacked_tree)) {
                all_stacked_trees.push(stacked_tree);
                let button = document.createElement("button");
                button.id = stacked_tree;
                button.innerText = stacked_tree;
                button.onclick = stacked_tree_button_click;
                buttons.appendChild(button);
                let this_button = document.getElementById(stacked_tree);
                this_button.setAttribute("class", "stackedtreebutton");
            }
        }
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
function reset_view() {
    let swap = {".linktree": "link",
                ".linkstackedtree": "link",
                ".noderoot": "node",
                ".noderootborder": "nodeborder",
                ".nodestacked": "node",
                ".nodestackedborder": "nodeborder"};
    for (item in swap) {
        let elements = document.querySelectorAll(item);
        if (elements.length > 0 ) { for (e of elements) { e.setAttribute("class", swap[item]); } }
    }
}
function cell_click(evt) {
    reset_view();
    let bordernodes = document.querySelectorAll(".noderootborder");
    for (node of bordernodes) { node.setAttribute("class", "nodeborder"); }
    let c = evt.target.getAttribute("class");
    if ( c == "node") {
        evt.target.setAttribute("class", "noderoot");
    } else if ( c == "nodeborder" ) {
        evt.target.setAttribute("class", "noderootborder");
    }
    let my_id = evt.target.getAttribute("id");
    draw_black_tree("Tree:" + my_id);
}
function draw_black_tree(tree_id) {
    let count = 0;
    for (cell_id in cells) {
        let cell = cells[cell_id];
        let neighbors = cell.neighbors;
        if (cell.black_trees[tree_id]) {
            let tree = cell.black_trees[tree_id].tree;
            for (port in tree) {
                if (tree[port] == "Parent") {
                    let neighbor = neighbors[port];
                    let neighbor_name = neighbor.cell_name;
                    let neighbor_port = neighbor.port;
                    let link_id = make_link_id(cell_id, port, neighbor_name, neighbor_port);
                    let element = document.getElementById(link_id);
                    if (element) {
                        element.setAttribute("class", "linktree");
                    } else {
                        console.log("No element for ", link_id);
                    }
                }
            }
        }
    }
}
function stacked_tree_button_click(evt) {
    let stacked_tree_id = evt.target.getAttribute("id");
    let root_cell_id = find_root(stacked_tree_id);
    reset_view();
    draw_stacked_tree(root_cell_id, stacked_tree_id);
}
function draw_stacked_tree(cellID, stacked_tree_id){
    let cell = cells[cellID];
    let trees = cell.stacked_trees;
    let tree_test = trees.trees[stacked_tree_id];
    if (tree_test == null || tree_test == "undefined") { return; }
    let tree = tree_test.tree;
    let node = document.getElementById(cellID);
    if (node.getAttribute("class") == "node") {
        node.setAttribute("class", "nodestacked");
    } else if (node.getAttribute("class") == "nodeborder") {
        node.setAttribute("class", "nodestackedborder");
    }
    let neighbors = cell.neighbors;
    for (my_port in tree) {
        let neighbor_cell_id = neighbors[my_port].cell_name;
        if ( tree[my_port] == "Parent") {
            let neighbor = neighbors[my_port];
            if (has_stacked_tree(neighbor_cell_id, stacked_tree_id)) {
                let neighbor_port = neighbor.port;
                let link_id = make_link_id(cellID, my_port, neighbor_cell_id, neighbor_port);
                document.getElementById(link_id).setAttribute("class", "linkstackedtree");
            }
        }
        if ( tree[my_port] == "Child" ) {
            draw_stacked_tree(neighbor_cell_id, stacked_tree_id);
        }
    }
}
function has_stacked_tree(cellID, tree_id) {
    let tree = cells[cellID].stacked_trees.trees[tree_id];
    return tree != null && tree != "undefined"
}
function find_root(id) {
    for (cellID in cells) {
        let is_root = true;
        let stacked_tree = cells[cellID].stacked_trees.trees[id];
        if (stacked_tree != null && stacked_tree != "undefined") {
            for (port in stacked_tree.tree) {
                if (stacked_tree.tree[port] == "Parent") {
                    is_root = false;
                }
            }
        } else {
            is_root = false;
        }
        if (is_root) return cellID;
    }
    alert("No root cell found for " + id);
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
