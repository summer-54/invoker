#include "task.hpp"

#include <chrono>
#include <filesystem>
#include <iostream>
#include <random>

#include "lib/tar.hpp"

const std::string SOCKET_PATH = "/tmp/invoker.sock";
const std::string SOCKET_INNER_PATH = "/invoker.sock";
const std::string VOLUMES_ROOT = std::string(std::getenv("HOME")) + "/.invokerVolumes";

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

std::string taskNetworkName(const std::string& id, const std::string& network) {
    return "task-" + id + "-" + network + "-" + std::to_string(std::chrono::system_clock::now().time_since_epoch().count()) + "-" + randomstring(16);
}

Task::Task(const std::string& id, const std::string& tarBinaryData): id(id) {
    initToken = randomstring(256);
    auto imageTag = taskImageTag(id);
    podmanClient.buildTar(imageTag, tarBinaryData, "./Dockerfile");
    Tar tar(tarBinaryData);
    std::vector<std::string> networks;
    if (tar.contains("networks") == std::make_pair(true, false)) {
        std::istringstream stream(tar.extract("networks"));
        do {
            networks.emplace_back();
        } while (stream >> networks.back());
        networks.pop_back();
        for (const auto& network : networks) this->networks[network] = taskNetworkName(id, network);
        for (auto& network : networks) network = this->networks[network];
    }
    for (const auto& network : networks) podmanClient.createNetwork(network);
    volumePath = std::filesystem::path(VOLUMES_ROOT) / imageTag;
    if (!std::filesystem::exists(VOLUMES_ROOT)) std::filesystem::create_directory(VOLUMES_ROOT);
    std::filesystem::create_directory(volumePath);
    operatorContainer = podmanClient.run(imageTag, {}, {}, {{"INIT_TOKEN", initToken},
        {"SOCKET_PATH", SOCKET_INNER_PATH}}, {{SOCKET_PATH, SOCKET_INNER_PATH}, {volumePath, "/volume"}}, networks, "");
}

Task::~Task() = default;

void Task::stop() {

}

std::string Task::getToken() {
    return initToken;
}

std::map<std::string, std::string> Task::getNetworks() {
    return networks;
}

std::string Task::getVolumePath() {
    return volumePath;
}
