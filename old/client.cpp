#include <uv.h>
#include <string>
#include <functional>
#include <iostream>
#include <memory>

#include "socket.hpp"
#include "socketBase.hpp"

int main() {
    SocketBase::Client client("/tmp/myserver.sock");

    // Set up callbacks
    client.setDataCallback([](const std::string& data) {
        std::cout << "Server response: " << data << std::endl;
    });

    client.connect([&client](bool success) {
        if (success) {
            std::cout << "Connected to server!" << std::endl;

            // Send a test message
            // SocketBase::Client* clientPtr = static_cast<SocketBase::Client*>(uv_default_loop()->data);
            client.write("Hello from client!");
        } else {
            std::cerr << "Failed to connect to server" << std::endl;
        }
    });

    // Store the client pointer in the loop data for access in callbacks
    uv_default_loop()->data = &client;

    // Run the event loop
    client.run();
    //
    // return 0;
    // Socket::connect("/tmp/myserver.sock", [](Socket::Socket& socket) {
    //     std::cout << "Connected\n";
    //     socket.write("Hello from client");
    //     socket.onData([&socket](const std::string& data) {
    //         std::cout << "Received: " << data << '\n';
    //     });
    // });
}