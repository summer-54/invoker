#include "podmanClient.hpp"
#include <httplib.h>
#include <nlohmann/json.hpp>
#include <iostream>
#include <stdexcept>

struct PodmanClient::Impl {
    httplib::Client cli_;
    std::function<void(const std::string&)> onStdoutCallback_;
    std::function<void(const std::string&)> onStderrCallback_;

    Impl(const std::string& socket_path) : cli_(socket_path) {}
};

PodmanClient::PodmanClient(const std::string& socket_path) : pimpl_(std::make_unique<Impl>(socket_path)) {}
PodmanClient::~PodmanClient() = default;

void PodmanClient::build_image(const std::string& context, const std::string& dockerfilePath, const std::string& tag) {
    std::string url = "/build?dockerfile=" + dockerfilePath + "&t=" + tag;
    auto res = pimpl_->cli_.Post(url, context, "application/x-tar",
                                 [](const char *data, size_t data_length) {
                                     std::cout << std::string(data, data_length);
                                     return true;
                                 });
    if (!res || res->status != 200) {
        throw std::runtime_error("Failed to build image");
    }
    std::cout << "Image built successfully: " << tag << std::endl;
}

std::string PodmanClient::create_container(const std::string& image, const std::vector<std::string>& cmd,
                                           const std::map<std::string, std::string>& ports,
                                           const std::map<std::string, std::string>& env,
                                           const std::vector<std::pair<std::string, std::string>>& volumes,
                                           const std::string& initStdin) {
    nlohmann::json body = {
        {"Image", image},
        {"Cmd", cmd},
        {"OpenStdin", true},
        {"Tty", true},
        {"AttachStdout", true},
        {"AttachStderr", true}
    };

    // Configure ExposedPorts
    nlohmann::json exposed_ports = nlohmann::json::object();
    for (const auto& p : ports) {
        std::string container_port = p.first + "/tcp";
        exposed_ports[container_port] = nlohmann::json::object();
    }
    body["ExposedPorts"] = exposed_ports;

    // Configure Environment Variables
    std::vector<std::string> env_list;
    for (const auto& e : env) {
        env_list.push_back(e.first + "=" + e.second);
    }
    body["Env"] = env_list;

    // Configure HostConfig for port bindings and volumes
    nlohmann::json host_config = {
        {"PortBindings", nlohmann::json::object()},
        {"Mounts", nlohmann::json::array()}
    };
    for (const auto& p : ports) {
        std::string container_port = p.first + "/tcp";
        host_config["PortBindings"][container_port] = {
            {{"HostPort", p.second}}
        };
    }
    for (const auto& v : volumes) {
        host_config["Mounts"].push_back({
            {"Type", "bind"},
            {"Source", v.first},
            {"Target", v.second}
        });
    }
    body["HostConfig"] = host_config;

    auto res = pimpl_->cli_.Post("/containers/create", body.dump(), "application/json");
    if (!res || res->status != 201) {
        throw std::runtime_error("Failed to create container");
    }
    auto json_res = nlohmann::json::parse(res->body);
    std::string container_id = json_res["Id"];
    std::cout << "Container created with ID: " << container_id << std::endl;
    return container_id;
}

void PodmanClient::start_container(const std::string& container_id) {
    auto res = pimpl_->cli_.Post("/containers/" + container_id + "/start", "");
    if (!res || res->status != 204) {
        throw std::runtime_error("Failed to start container");
    }
    std::cout << "Container started: " << container_id << std::endl;
}

std::string PodmanClient::run_container(const std::string& image, const std::vector<std::string>& cmd) {
    std::string container_id = create_container(image, cmd, {}, {}, {}, "");
    start_container(container_id);
    return container_id;
}

void PodmanClient::stop_container(const std::string& container_id) {
    auto res = pimpl_->cli_.Post("/containers/" + container_id + "/stop", "");
    if (!res || res->status != 204) {
        throw std::runtime_error("Failed to stop container");
    }
    std::cout << "Container stopped: " << container_id << std::endl;
}

void PodmanClient::restart_container(const std::string& container_id) {
    auto res = pimpl_->cli_.Post("/containers/" + container_id + "/restart", "");
    if (!res || res->status != 204) {
        throw std::runtime_error("Failed to restart container");
    }
    std::cout << "Container restarted: " << container_id << std::endl;
}

void PodmanClient::write_to_container_stdin(const std::string& container_id, const std::string& input) {
    auto res = pimpl_->cli_.Post("/containers/" + container_id + "/attach?stdin=1&stream=1",
                                 input,
                                 "application/vnd.docker.raw-stream");
    if (!res || res->status != 200) {
        throw std::runtime_error("Failed to write to container stdin");
    }
    std::cout << "Wrote to container stdin: " << container_id << std::endl;
}

void PodmanClient::setOnStdoutCallback(std::function<void(const std::string&)> callback) {
    pimpl_->onStdoutCallback_ = callback;
}

void PodmanClient::setOnStderrCallback(std::function<void(const std::string&)> callback) {
    pimpl_->onStderrCallback_ = callback;
}

void PodmanClient::attach_to_container(const std::string& container_id) {
    auto res = pimpl_->cli_.Post("/containers/" + container_id + "/attach?stdout=1&stderr=1&stream=1", "",
                                 "application/vnd.docker.raw-stream",
                                 [this](const char *data, size_t data_length) {
                                     std::string output(data, data_length);
                                     if (pimpl_->onStdoutCallback_) pimpl_->onStdoutCallback_(output);
                                     if (pimpl_->onStderrCallback_) pimpl_->onStderrCallback_(output);
                                     return true;
                                 });
    if (!res || res->status != 200) {
        throw std::runtime_error("Failed to attach to container");
    }
}