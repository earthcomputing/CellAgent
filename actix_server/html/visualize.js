const x0 = 50;
const y0 = 50;
const scale = 50;
let cells = {};
let allNeighbors = {};
let canvas;
window.onload = function() {
    canvas = document.getElementById("viz-canvas");
}
function visualize() {
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
                }
                draw();
            }

        }
    }
}
function setup_geometry(geometry_text) {
    let geometry = JSON.parse(geometry_text);
    let rowcol = geometry.geometry.rowcol;
    for (cell in rowcol) {
        cells[cell] = rowcol[cell];
    }
}
function setup_topology(topology_text) {
    let topology = JSON.parse(topology_text);
    allNeighbors = topology.neighbors;
    for (cellID in allNeighbors) {
        let cellNeighbors = allNeighbors[cellID];
        for (neighborIndex in cellNeighbors.neighbors) {
            let neighbor = cellNeighbors.neighbors[neighborIndex];
            let neighborID = neighbor.cell_id.name;
            if ( cellID < neighborID ) {
                let neighborPort = neighbor.port;
                let id = cellID + ":P"+ neighborIndex + "-" + neighborID + ":P" + neighborPort;
                create_line_at(id, cellID, neighborID);
                for (cell in cells) {
                    create_node_at(cell);
                }
            }
        }
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
    canvas.appendChild(line);
    return line;
}
function create_node_at(id) {
    let x = cells[id].col;
    let y = cells[id].row;
    let circle = document.createElement("circle");
    circle.setAttribute("id", id);
    circle.setAttribute("class", "node");
    circle.setAttribute("cx", x0 + x*scale);
    circle.setAttribute("cy", y0 + y*scale);
    canvas.appendChild(circle);
    return circle;
}
function draw() { canvas.innerHTML = canvas.innerHTML; }
