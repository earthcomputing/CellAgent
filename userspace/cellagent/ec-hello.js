var express = require('express');
var url = require('url');
var app = express();

app.get('/', function (req, res) {
   res.send("EARTH Computing Hello World");
})

app.post('/', function(req, res, body) {
    console.log("Got req 3")
    console.log(req.body);
    res.send("Response");
})                

// This responds with "Hello World" on the homepage
app.get('/', function (req, res) {
   console.log("Got a GET request for the homepage");
   res.send('Hello GET');
})

var server = app.listen(8081, function () {
   var host = server.address().address
   var port = server.address().port

   console.log("Example app listening at http://%s:%s", host, port)
})
