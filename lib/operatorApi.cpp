#include "operatorApi.hpp"

#include <iostream>
#include <memory>
#include <thread>
#include <utility>
#include <chrono>

OperatorApi::OperatorApi(Socket::Connection* connection): connection(connection) {
}

void OperatorApi::init() {
    connection->onData([self = shared_from_this()](const char* chunk, int length) {
        std::string data(chunk, length);
        for (const auto& callback : self->callbacks) {
            callback(data);
        }
    });
}

std::string OperatorApi::stringValue(const STDOUT value) {
    switch (value) {
        case STDOUT::none: return "none";
        case STDOUT::onEnd: return "onEnd";
        case STDOUT::normal: return "normal";
    }
    return "normal";
}

std::string OperatorApi::stringValue(const Verdict value) {
    switch (value) {
        case Verdict::OK: return "OK";
        case Verdict::WA: return "WA";
        case Verdict::TL: return "TL";
        case Verdict::ML: return "ML";
        case Verdict::ITL: return "ITL";
        case Verdict::RTL: return "RTL";
        case Verdict::RML: return "RML";
        case Verdict::CE: return "CE";
        case Verdict::ERR: return "ERR";
    }
    return "0";
}

OperatorApi::ContainerTemplate::ContainerTemplate(const int image, const std::shared_ptr<OperatorApi> operatorApi): image(image), operatorApi(operatorApi) {}

void OperatorApi::ContainerTemplate::onStdout(const std::function<void(const std::string&)>& callback) const {
    operatorApi->callbacks.emplace_back([this, callback](const std::string& chunk) {
        if (chunk.starts_with("STDOUT")) callback(chunk.substr(7));
    });
}

void OperatorApi::ContainerTemplate::onStderr(const std::function<void(const std::string&)>& callback) const {
    operatorApi->callbacks.emplace_back([this, callback](const std::string& chunk) {
        if (chunk.starts_with("STDERR")) callback(chunk.substr(7));
    });
}

OperatorApi::Container* OperatorApi::ContainerTemplate::run() {
    std::ostringstream stream;
    int id = operatorApi->containersCount++;
    stream << "RUN " << id << '\n' << image << '\n' << "STDOUT " << stringValue(stdout) << '\n'
        << "STDERR " << stringValue(stderr) << '\n';
    for (const auto& [a, b] : volumes) stream << "VOLUME " << a << '\n' << b << '\n';
    for (const auto& [a, b] : env) stream << "ENV " << a << ' ' << b << '\n';
    for (const auto& network : networks) stream << "NETWORK " << network << '\n';
    if (!initStdin.empty()) stream << "WRITE\n" << initStdin;
    std::string chunk = stream.str();
    chunk.pop_back();
    operatorApi->connection->write(chunk);
    return new OperatorApi::Container(id, this, operatorApi);
}

OperatorApi::Container::Container(const int id, ContainerTemplate* containerTemplate, const std::shared_ptr<OperatorApi> operatorApi): id(id), containerTemplate(containerTemplate), operatorApi(operatorApi) {}

void OperatorApi::Container::onStdout(const std::function<void(const std::string&)>& callback) const {
    operatorApi->callbacks.emplace_back([this, callback](const std::string& chunk) {
        if (chunk.starts_with("STDOUT")) callback(chunk.substr(7));
    });
}

void OperatorApi::Container::onStderr(const std::function<void(const std::string&)>& callback) const {
    operatorApi->callbacks.emplace_back([this, callback](const std::string& chunk) {
        if (chunk.starts_with("STDERR")) callback(chunk.substr(7));
    });
}

void OperatorApi::Container::restart() const {
    operatorApi->connection->write("RESTART " + std::to_string(id));
}

void OperatorApi::Container::stop() const {
    operatorApi->connection->write("STOP " + std::to_string(id));
}

void OperatorApi::Container::write(const std::string& chunk) const {
    operatorApi->connection->write("RESTART " + std::to_string(id) + '\n' + chunk);
}

void OperatorApi::Container::getHost(const std::function<void(const std::string&)>& callback) const {
    auto state = std::make_shared<std::pair<bool, std::function<void(const std::string&)>>>(std::make_pair(false, callback));
    operatorApi->callbacks.emplace_back([this, state](const std::string& chunk) {
        if (!state->first && chunk.starts_with("HOST")) {
            state->second(chunk.substr(5));
            state->first = true;
        }
    });
    operatorApi->connection->write("HOST " + std::to_string(id));
}

void OperatorApi::create(const std::string& path, const std::string& initToken, const std::function<void(std::shared_ptr<OperatorApi>)> callback) {
    Socket::Client client;
    auto connection = client.connect(path.c_str());
    connection->onConnected([&connection, &callback, initToken] {
        std::this_thread::sleep_for(std::chrono::milliseconds(10));
        connection->write(initToken);
        auto operatorApi = std::shared_ptr<OperatorApi>(new OperatorApi(connection));
        operatorApi->init();
        callback(operatorApi);
    });
    client.run();
}

std::function<OperatorApi::ContainerTemplate*(std::shared_ptr<OperatorApi>)> OperatorApi::build(const std::string& context, const std::string& dockerfilePath) {
    int image = imagesCount++;
    connection->write("BUILD " + std::to_string(image) + '\n' + context + '\n' + dockerfilePath);
    return [image](std::shared_ptr<OperatorApi> operatorApi) {
        return new ContainerTemplate(image, operatorApi);
    };
}

void OperatorApi::setVerdict(const std::string& subtaskId, Verdict verdict, const std::string& data) const {
    connection->write("VERDICT " + stringValue(verdict) + "\nSUB " + subtaskId + (data.empty() ? "" : "\nDATA" + data));
}

void OperatorApi::setVerdict(Verdict verdict, const std::string& data) const {
    connection->write("VERDICT " + stringValue(verdict) + (data.empty() ? "" : "\nDATA" + data));
}