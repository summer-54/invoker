import * as fs from "node:fs";

Bun.serve({
    hostname: "localhost",
    port: 9000,
    fetch(req, server) {
        const url = new URL(req.url);

        // Handle WebSocket upgrade for /invoker
        if (url.pathname === '/invoker' && req.headers.get('upgrade') === 'websocket') {
            const success = server.upgrade(req);
            return success ? undefined : new Response('WebSocket upgrade failed', { status: 400 });
        }

        // Default HTTP response for other routes
        return new Response('Hello World');
    },

    websocket: {
        open(ws) {
            console.log('WebSocket connection opened');
            // ws.send('Welcome to /invoker WebSocket!');
            ws.send("0\nSTART\n" + fs.readFileSync("./test.tar.json"))
        },

        message(ws, message) {
            console.log('Received:', message);
            // Echo the message back
            ws.send(`Echo: ${message}`);
        },

        close(ws, code, message) {
            console.log('WebSocket connection closed');
        }
    }
});