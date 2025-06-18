#include <memory>
#include <iostream>
#include "socket.hpp"

int main() {
    try {
        Socket::serve("/tmp/myserver.sock", [](Socket::Socket& socket) {
            std::cout << "New connection\n";
            socket.write("Hello from server");
            socket.onData([&socket](const std::string& data) {
                std::cout << "Received: " << data << '\n';
                socket.write("Hello from server2");
            });
        });
    } catch (const std::exception& e) {
        std::cerr << "Error: " << e.what() << std::endl;
        return 1;
    }
}