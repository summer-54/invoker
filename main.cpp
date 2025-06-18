#include <iostream>
#include <string>
#include <thread>

#include "asyncProcess.hpp"

int main() {
    try {
        // AsyncProcess process("python -i");
        //
        // process.onStdout([](const std::string& data) {
        //     std::cout << data;
        // });
        //
        // process.onStderr([](const std::string& data) {
        //     std::cerr << data;
        // });
        //
        // process.onEnd([](int exit_code) {
        //     std::cout << "\nProcess ended with code: " << exit_code << std::endl;
        // });
        //
        // // Give Python time to start
        // // std::this_thread::sleep_for(std::chrono::milliseconds(500));
        //
        // // Use the public in stream for synchronous writes
        // process.in << "print('Hello from Python!')" << std::endl;
        // process.in << "x = 42" << std::endl;
        // process.in << "print(f'x squared is {x*x}')" << std::endl;
        // process.in << "exit()" << std::endl;
        //
        // // Keep main thread alive while process runs
        // while (process.running()) {
        //     std::this_thread::sleep_for(std::chrono::milliseconds(100));
        // }
        AsyncProcess process("tree", "../../..");
        process.onStdout([](auto data) {
            std::cout << data;
        });
        while (process.running()) {
            std::this_thread::sleep_for(std::chrono::milliseconds(100));
        }

    } catch (const std::exception& e) {
        std::cerr << "Error: " << e.what() << std::endl;
        return 1;
    }

    return 0;
}

// int main() {
    // Socket::Server server("/tmp/mySocket");
    // server.onConnect([](Socket::Connection* conn) {
    //     conn->write("Hello from server");
    //     conn->onData([](const char* data, size_t len) {
    //         std::cout << "Received: " << std::string(data, len) << std::endl;
    //     });
    //     conn->onClose([]() {
    //         std::cout << "Connection closed" << std::endl;
    //     });
    // });
    // server.start();
// }