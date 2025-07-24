// Client connection to Unix socket
const socket = await Bun.connect({
    unix: "/invoker.sock", // path to Unix socket
    socket: {
        open(socket) {
            console.log("Connected to Unix socket");
        },
        data(socket, data) {
            console.log("Received:", data.toString());
        },
        close(socket) {
            console.log("Connection closed");
        },
        error(socket, error) {
            console.error("Socket error:", error);
        }
    }
});

// Send data
socket.write("Hello Unix socket!");