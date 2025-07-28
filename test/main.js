import * as http from "node:http";

const server = http.createServer();

server.on("request", (request, response) => {
    console.log(request);
    response.writeHead(200);
    response.end("test");
});

server.listen(50000);