#include <iostream>
#include "lib/lib/socket.hpp"

int main() {
    Socket::Client client;
    auto conn = client.connect("/tmp/mySocket");
    conn->write("Hello from client");
    conn->onData([](const char* data, size_t len) {
        std::cout << "Received: " << std::string(data, len) << std::endl;
    });
    conn->onClose([]() {
        std::cout << "Connection closed" << std::endl;
    });
    client.run();
}