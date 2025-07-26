#include <iostream>

#include "lib/operatorApi.hpp"

int main() {
    // OperatorApi operatorApi("/tmp/mySocket");
    OperatorApi::create(std::getenv("SOCKET_PATH"), std::getenv("INIT_TOKEN"), [](std::shared_ptr<OperatorApi> operatorApi) {
        std::cerr << std::getenv("SOCKET_PATH") << std::endl << std::getenv("INIT_TOKEN") << std::endl;
        auto image = operatorApi->build("/home/sizoff/programming/invoker/test", "./Dockerfile");
        auto template_ = image();
        // template_->env["TEST"] = "true";
        // template_->ports.push_back(80);
        // template_->volumes.emplace_back("/home/sizoff/testVolume", "/volume");
        template_->networks.push_back("test1");
        // (*template_) << "test";
        auto container = template_->run();
        // (*container) << "test";
        // container->getPort(80, [](int){});
        // container->stop();
        // container->restart();
        // operatorApi->setVerdict("16", OperatorApi::Verdict::OK);
    });
}