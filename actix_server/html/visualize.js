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
    canvas.innerHTML =
        '<defs>\
             <!-- From http://stackoverflow.com/questions/26789005/drawing-arrows-using-d3 -->\
             <marker id="arrow-head" markerWidth="10" markerHeight="10"\
                   refx="10" refy="3" orient="auto" markerUnits="strokeWidth" viewBox="0 0 20 20">\
               <path d="M0,0 L0,6 L9,3 z" fill="black"/>\
             </marker>\
         </defs>';
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
                }
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
            }
        }
    }
    for (cellID in allNeighbors) {
        create_node_at(cellID);
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
    circle.setAttribute("onclick", "cell_click(evt)");
    circle.setAttribute("ondblclick", "cell_dblclick(evt)");
    canvas.appendChild(circle);
    return circle;
}
function draw() { canvas.innerHTML = canvas.innerHTML; }
function cell_click(evt) {
    if (evt.target.getAttribute("class") == "node") {
        evt.target.setAttribute("class", "noderoot");
    } else {
        evt.target.setAttribute("class", "node");
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