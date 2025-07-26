#include <chrono>
#include <fstream>
#include <httplib.h>
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
    try {
        std::vector<Session*> sessions;
        Socket::Server server(SOCKET_PATH.c_str());
        WebSocketClient client("ws://localhost:9000/invoker");
        client.connect();
        server.onConnect([&sessions, &client](Socket::Connection* conn) {
            auto connPtr = std::make_shared<Socket::Connection*>(conn);
            conn->onData([&client, connPtr, &sessions](const char* chunk, size_t size) {
                std::string data(chunk, size);
                if ((*connPtr)->data != nullptr) {
                    auto* session = static_cast<Session*>((*connPtr)->data);
                    session->onData(data);
                    return;
                }
                for (const auto& task : client.tasks_ | std::views::values) {
                    if (task->getToken() == data) {
                        sessions.push_back(new Session());
                        task->session = sessions.back();
                        (*connPtr)->data = sessions.back();
                    }
                }
            });
            conn->onClose([] {
                std::cout << "Connection closed" << '\n' << std::endl;
            });
        });
        server.start([&client] {
            // Task task("0", readFileToString("../test0.tar.gz"));
            std::cout << "started" << std::endl;
        });
        return 0;
    } catch (const std::exception& e) {
        std::cerr << "Unhandled exception: " << e.what() << std::endl;
        return 1;
    } catch (...) {
        std::cerr << "Unhandled unknown exception" << std::endl;
        return 1;
    }
}