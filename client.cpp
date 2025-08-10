#include <iostream>

#include "lib/operatorApi.hpp"

int main() {
    // OperatorApi operatorApi("/tmp/mySocket");
    OperatorApi::create(std::getenv("SOCKET_PATH"), std::getenv("INIT_TOKEN"), [](std::shared_ptr<OperatorApi> operatorApi) {
        auto image = operatorApi->build("/home/sizoff/programming/invoker/test", "./Dockerfile");
        auto template_ = image(operatorApi);
        template_->env["TEST"] = "true";
        // template_->ports.push_back(80);
        template_->volumes.emplace_back("test", "/volume");
        template_->networks.emplace_back("test1");
        // (*template_) << "test";
        auto container = template_->run();
        // (*container) << "test";
        // container->getPort(80, [](int){});
        // container->stop();
        // container->restart();
        container->getHost([](const std::string& host) {
            std::cout << host << std::endl;
        });
        // operatorApi->setVerdict("16", OperatorApi::Verdict::OK);
    });
}