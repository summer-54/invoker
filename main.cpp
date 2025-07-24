#include <chrono>
#include <fstream>
#include <iostream>
#include <sstream>

#include "lib/lib/socket.hpp"
#include "session.hpp"
#include "task.hpp"
// #include "websocketClient.hpp"

std::string readFileToString(const std::string& filename) {
    std::ifstream file(filename, std::ios::binary | std::ios::ate);
    if (!file) {
        throw std::runtime_error("Cannot open file: " + filename);
    }

    // Get file size
    std::streamsize size = file.tellg();
    file.seekg(0, std::ios::beg);

    // Read file content
    std::string buffer(size, '\0');
    if (!file.read(&buffer[0], size)) {
        throw std::runtime_error("Error reading file: " + filename);
    }

    return buffer;
}

int main() {
    std::vector<Session*> sessions;
    Socket::Server server(SOCKET_PATH.c_str());
    // WebSocketClient client("ws://localhost:9000/invoker");
    server.onConnect([&sessions](Socket::Connection* conn) {
        sessions.push_back(new Session(conn));
    });
    server.start([] {
        Task task("0", readFileToString("../test.tar.gz"));
    });
}