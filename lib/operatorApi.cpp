#include "operatorApi.hpp"
#include <utility>

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

OperatorApi::ContainerTemplate::ContainerTemplate(const int image, OperatorApi* operatorApi): image(image), operatorApi(operatorApi) {}

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
    stream << "RUN " << id << '\n' << "STDOUT " << stringValue(stdout) << '\n'
        << "STDERR " << stringValue(stderr) << '\n';
    if (!ports.empty()) {
        stream << "PORTS";
        for (int port : ports) stream << ' ' << port;
        stream << '\n';
    }
    for (const auto& [a, b] : volumes) stream << "VOLUME " << a << ' ' << b << '\n';
    for (const auto& [a, b] : env) stream << "ENV " << a << ' ' << b << '\n';
    if (!initStdin.empty()) stream << "WRITE\n" << initStdin;
    std::string chunk = stream.str();
    chunk.pop_back();
    operatorApi->connection->write(chunk);
    return new OperatorApi::Container(id, this, operatorApi);
}

OperatorApi::Container::Container(const int id, ContainerTemplate* containerTemplate, OperatorApi* operatorApi): id(id), containerTemplate(containerTemplate), operatorApi(operatorApi) {}

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

void OperatorApi::Container::getPort(int port, const std::function<void(int)>& callback) const {
    bool found = false;
    operatorApi->callbacks.emplace_back([this, &callback, &found](const std::string& chunk) {
        if (!found && chunk.starts_with("PORT")) {
            callback(std::stoi(chunk.substr(5)));
            found = true;
        }
    });
    operatorApi->connection->write("PORT " + std::to_string(id) + '\n' + std::to_string(port));
}

OperatorApi::OperatorApi(const std::string& socket) {
    client = new Socket::Client;
    connection = client->connect(socket.c_str());
    connection->onData([this](const char* chunk, int length) {
        std::string data(chunk, length);
        for (const auto& callback : this->callbacks) {
            callback(data);
        }
    });
}

std::function<OperatorApi::ContainerTemplate*()> OperatorApi::build(const std::string& context, const std::string& dockerfilePath) {
    int image = imagesCount++;
    connection->write("BUILD " + std::to_string(image) + '\n' + context + '\n' + dockerfilePath);
    return [&image, this] {
        return new ContainerTemplate(image, this);
    };
}

void OperatorApi::setVerdict(const std::string& subtaskId, Verdict verdict, const std::string& data) const {
    connection->write("VERDICT " + stringValue(verdict) + "\nSUB " + subtaskId + "\nDATA" + data);
}

void OperatorApi::setVerdict(Verdict verdict, const std::string& data) const {
    connection->write("VERDICT " + stringValue(verdict) + "\nDATA" + data);
}