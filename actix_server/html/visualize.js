function visualize() {
    let canvas = document.getElementById("viz-canvas");
    //canvas.innerHTML = '<line id="line" class="link" x1="50" y1="50" x2="50" y2="100">';
    let line = document.createElement("line");
    line.setAttribute("id", "line");
    line.setAttribute("class", "link");
    line.setAttribute("x1", "50");
    line.setAttribute("y1", "50");
    line.setAttribute("x2", "50");
    line.setAttribute("y2", "100");
    canvas.appendChild(line);
    //canvas.innerHTML = '<circle id="circle" class="node" cx="50" cy="50" r="40">';
    let circle = document.createElement("circle");
    circle.setAttribute("id", "circle");
    circle.setAttribute("class", "node");
    circle.setAttribute("cx", "50");
    circle.setAttribute("cy", "50");
    circle.setAttribute("r", "10");
    canvas.appendChild(circle);
    canvas.innerHTML = canvas.innerHTML;
    const Http = new XMLHttpRequest();
    const url = 'http://127.0.0.1:8088/geometry';
    Http.open("GET", url);
    Http.send();
    Http.onreadystatechange = (e) => {
        if ( Http.readyState == 4 && Http.status == 200 ) {
            draw_geometry(Http.responseText);
        }
    }
}
function draw_geometry(geometry_text) {
        let geometry = JSON.parse(geometry_text);
        console.log(geometry.geometry)
}