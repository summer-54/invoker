#include "task.hpp"

#include <chrono>
#include <iostream>
#include <random>

const std::string SOCKET_PATH = "/tmp/invoker.sock";
const std::string SOCKET_INNER_PATH = "/invoker.sock";

std::string randomstring(size_t length) {
    const std::string charset = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    std::random_device rd;
    std::mt19937 generator(rd());
    std::uniform_int_distribution<> distribution(0, charset.size() - 1);

    std::string result;
    result.reserve(length);

    for (size_t i = 0; i < length; ++i) {
        result += charset[distribution(generator)];
    }

    return std::to_string(std::chrono::system_clock::now().time_since_epoch().count()) + result;
}

std::string taskImageTag(const std::string& id) {
    return "task-" + id + "-" + std::to_string(std::chrono::system_clock::now().time_since_epoch().count());
}

Task::Task(const std::string& id, const std::string& tarBinaryData): id(id) {
    initToken = randomstring(256);
    auto imageTag = taskImageTag(id);
    podmanClient.buildTar(imageTag, tarBinaryData, "./Dockerfile");
    operatorContainer = podmanClient.run(imageTag, {}, {}, {{"INIT_TOKEN", initToken},
        {"SOCKET_PATH", SOCKET_INNER_PATH}}, {{SOCKET_PATH, SOCKET_INNER_PATH}}, {}, "");
}

Task::~Task() = default;

void Task::tryConnection(const std::string& init, Socket::Connection* connection) {
    std::cerr << init << '\n' << initToken << std::endl;
    if (init != initToken) return;
    session = new Session(connection);
}

void Task::stop() {

}

std::string Task::getToken() {
    return initToken;
}
