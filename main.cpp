#include <chrono>
#include <fstream>
#include <iostream>
#include <ranges>
#include <sstream>

#include "lib/lib/socket.hpp"
#include "session.hpp"
#include "task.hpp"
#include "websocketClient.hpp"

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
    WebSocketClient client("ws://localhost:9000/invoker");
    client.connect();
    server.onConnect([&sessions, &client](Socket::Connection* conn) {
        // sessions.push_back(new Session(conn));
        conn->onData([&client, &conn, &sessions](const char* chunk, size_t size) {
            std::string token(chunk, size);
            for (const auto& task : client.tasks_ | std::views::values) {
                if (task->getToken() == token) {
                    sessions.push_back(new Session(conn));
                    task->session = sessions.back();
                }
            }
        });
    });
    server.start([&client] {
        // Task task("0", readFileToString("../test0.tar.gz"));
        std::cout << "started";
    });
}