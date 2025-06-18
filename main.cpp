#include <iostream>
#include "socket.hpp"

int main() {
    Socket::Server server("/tmp/mySocket");
    server.onConnect([](Socket::Connection* conn) {
        conn->write("Hello from server");
        conn->onData([](const char* data, size_t len) {
            std::cout << "Received: " << std::string(data, len) << std::endl;
        });
        conn->onClose([]() {
            std::cout << "Connection closed" << std::endl;
        });
    });
    server.start();
}