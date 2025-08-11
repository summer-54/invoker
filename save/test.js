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
            const fileData = fs.readFileSync("./test.tar.gz");
            const message = new TextEncoder().encode("0\nSTART\n"); // Encode text prefix
            const combined = new Uint8Array([...message, ...fileData]); // Combine prefix and binary data
            ws.send(combined); // Send as binary
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