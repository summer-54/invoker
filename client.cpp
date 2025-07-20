// #include <iostream>
// #include "lib/lib/socket.hpp"
//
// int main() {
//     Socket::Client client;
//     auto conn = client.connect("/tmp/mySocket");
//     conn->write("Hello from client");
//     conn->onData([](const char* data, size_t len) {
//         std::cout << "Received: " << std::string(data, len) << std::endl;
//     });
//     conn->onClose([]() {
//         std::cout << "Connection closed" << std::endl;
//     });
//     client.run();
// }

#include <iostream>

#include "lib/operatorApi.hpp"

int main() {
    // OperatorApi operatorApi("/tmp/mySocket");
    OperatorApi::create("/tmp/mySocket", [](OperatorApi operatorApi) {
        auto image = operatorApi.build("/volume/solution", "/volume/solution/Dockerfile");
        auto template_ = image();
        template_->env["TEST"] = "true";
        template_->ports.push_back(80);
        template_->volumes.emplace_back("/volume/test", "/test");
        // *template_ << "test";
        auto container = template_->run();
        // *container << "test";
        container->getPort(80, [](int){});
        container->stop();
        container->restart();
        operatorApi.setVerdict("16", OperatorApi::Verdict::OK);
    });
}