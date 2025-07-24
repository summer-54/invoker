#include "podmanClient.hpp"
#include <httplib.h>
#include <nlohmann/json.hpp>
#include <archive.h>
#include <archive_entry.h>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <stdexcept>
#include <utility>
#include <vector>

namespace {
    long int write_callback(struct archive *a, void *user_data, const void *buffer, size_t length) {
        auto* data = static_cast<std::vector<char>*>(user_data);
        const char* buf = static_cast<const char*>(buffer);
        data->insert(data->end(), buf, buf + length);
        return ARCHIVE_OK;
    }
}

struct PodmanClient::Impl {
    httplib::Client cli_;
    std::map<std::string, std::function<void(const std::string&)>> onStdoutCallbacks_;
    std::map<std::string, std::function<void(const std::string&)>> onStderrCallbacks_;

    explicit Impl(const std::string& socket_path) : cli_(socket_path) {}
};

PodmanClient::PodmanClient(const std::string& socket_path) : pimpl_(std::make_unique<Impl>(socket_path)) {}
PodmanClient::~PodmanClient() = default;

void PodmanClient::buildTar(const std::string& tag, const std::string& binaryTarData, const std::string& dockerfilePath) const {
    std::string url = "/build?t=" + tag + "&dockerfile=" + dockerfilePath;
    auto res = pimpl_->cli_.Post(
        url,
        binaryTarData.size(),
        [&binaryTarData](uint64_t offset, uint64_t length, httplib::DataSink& sink) {
            sink.write(binaryTarData.data() + offset, std::min(length, binaryTarData.size() - offset));
            return true;
        },
        "application/x-tar"
    );

    if (!res) {
        std::cerr << "Build failed: " << res.error() << std::endl;
    } else if (res->status != 200) {
        std::cerr << "Build failed with status: " << res->status << std::endl;
    } else {
        std::cout << res->body << std::endl;
    }
}

void PodmanClient::build(const std::string& tag, const std::string& context, const std::string& dockerfilePath) const {
    // Validate context is a directory
    if (!std::filesystem::is_directory(context)) {
        throw std::runtime_error("Context path is not a directory: " + context);
    }

    // Create in-memory tar archive
    std::vector<char> tar_buffer;
    struct archive *a = archive_write_new();
    archive_write_set_format_ustar(a);
    archive_write_open(a, &tar_buffer, nullptr, write_callback, nullptr);

    // Recursively add files from context directory
    for (const auto& entry : std::filesystem::recursive_directory_iterator(context)) {
        std::filesystem::path path = entry.path();
        std::string rel_path = std::filesystem::relative(path, context).string();
        if (rel_path == ".") continue; // Skip the context directory itself

        struct archive_entry *ae = archive_entry_new();
        archive_entry_set_pathname(ae, rel_path.c_str());
        archive_entry_set_size(ae, std::filesystem::file_size(path));
        archive_entry_set_filetype(ae, entry.is_directory() ? AE_IFDIR : AE_IFREG);
        archive_entry_set_perm(ae, 0644);
        archive_write_header(a, ae);

        if (!entry.is_directory()) {
            std::ifstream file(path, std::ios::binary);
            if (!file) {
                archive_entry_free(ae);
                archive_write_free(a);
                throw std::runtime_error("Failed to open file: " + path.string());
            }
            char buffer[8192];
            while (file) {
                file.read(buffer, sizeof(buffer));
                archive_write_data(a, buffer, file.gcount());
            }
        }

        archive_entry_free(ae);
    }

    archive_write_close(a);
    archive_write_free(a);

    // Convert tar_buffer to std::string and call buildTar
    std::string tar_data(tar_buffer.begin(), tar_buffer.end());
    buildTar(tag, tar_data, dockerfilePath);
}

std::string PodmanClient::create(const std::string& image, const std::vector<std::string>& cmd,
                                           const std::map<std::string, std::string>& ports,
                                           const std::map<std::string, std::string>& env,
                                           const std::vector<std::pair<std::string, std::string>>& volumes,
                                           const std::vector<std::string>& networks) const {
    nlohmann::json body = {
        {"Image", image},
        {"Cmd", cmd},
        {"OpenStdin", true},
        {"Tty", true},
        {"AttachStdout", true},
        {"AttachStderr", true}
    };

    nlohmann::json exposed_ports = nlohmann::json::object();
    for (const auto& key : ports | std::views::keys) {
        const std::string container_port = key + "/tcp";
        exposed_ports[container_port] = nlohmann::json::object();
    }
    body["ExposedPorts"] = exposed_ports;

    std::vector<std::string> env_list;
    for (const auto& [key, value] : env) {
        env_list.push_back(key + "=" + value);
    }
    body["Env"] = env_list;

    nlohmann::json host_config = {
        {"PortBindings", nlohmann::json::object()},
        {"Mounts", nlohmann::json::array()}
    };
    for (const auto& [host, virt] : ports) {
        const std::string container_port = host + "/tcp";
        host_config["PortBindings"][container_port] = {
            {{"HostPort", virt}}
        };
    }
    for (const auto& [host, virt] : volumes) {
        host_config["Mounts"].push_back({
            {"Type", "bind"},
            {"Source", host},
            {"Target", virt}
        });
    }
    body["HostConfig"] = host_config;

    if (!networks.empty()) {
        nlohmann::json endpoints_config = nlohmann::json::object();
        for (const auto& network : networks) {
            endpoints_config[network] = nlohmann::json::object();
        }
        body["NetworkingConfig"] = {
            {"EndpointsConfig", endpoints_config}
        };
    }

    auto res = pimpl_->cli_.Post("/containers/create", body.dump(), "application/json");
    if (!res || res->status != 201) {
        throw std::runtime_error("Failed to create container");
    }
    auto json_res = nlohmann::json::parse(res->body);
    std::string container_id = json_res["Id"];
    std::cout << "Container created with ID: " << container_id << std::endl;

    return container_id;
}

std::string PodmanClient::run(const std::string& image, const std::vector<std::string>& cmd,
                                        const std::map<std::string, std::string>& ports,
                                        const std::map<std::string, std::string>& env,
                                        const std::vector<std::pair<std::string, std::string>>& volumes,
                                        const std::vector<std::string>& networks,
                                        const std::string& initStdin) const {
    std::string container_id = create(image, cmd, ports, env, volumes, networks);
    start(container_id, initStdin);
    return container_id;
}

void PodmanClient::start(const std::string& container_id, const std::string& initStdin) const {
    auto res = pimpl_->cli_.Post("/containers/" + container_id + "/start");
    if (!res || res->status != 204) {
        throw std::runtime_error("Failed to start container");
    }
    if (!initStdin.empty()) {
        write(container_id, initStdin);
    }
    std::cout << "Container started: " << container_id << std::endl;
}

// std::string PodmanClient::run(const std::string& image, const std::vector<std::string>& cmd) {
//     return create(image, cmd, {}, {}, {}, "");
// }

void PodmanClient::stop(const std::string& container_id) const {
    auto res = pimpl_->cli_.Post("/containers/" + container_id + "/stop");
    if (!res || res->status != 204) {
        throw std::runtime_error("Failed to stop container");
    }
    std::cout << "Container stopped: " << container_id << std::endl;
}

void PodmanClient::restart(const std::string& container_id) const {
    auto res = pimpl_->cli_.Post("/containers/" + container_id + "/restart");
    if (!res || res->status != 204) {
        throw std::runtime_error("Failed to restart container");
    }
    std::cout << "Container restarted: " << container_id << std::endl;
}

void PodmanClient::write(const std::string& container_id, const std::string& input) const {
    auto res = pimpl_->cli_.Post("/containers/" + container_id + "/attach?stdin=1&stream=1",
                                 input,
                                 "application/vnd.docker.raw-stream");
    if (!res || res->status != 200) {
        throw std::runtime_error("Failed to write to container stdin");
    }
    std::cout << "Wrote to container stdin: " << container_id << std::endl;
}

void PodmanClient::onStdout(const std::string& container_id, std::function<void(const std::string&)> callback) const {
    pimpl_->onStdoutCallbacks_[container_id] = std::move(callback);
}

void PodmanClient::onStderr(const std::string& container_id, std::function<void(const std::string&)> callback) const {
    pimpl_->onStderrCallbacks_[container_id] = std::move(callback);
}

void PodmanClient::attach(const std::string& container_id) const {
    httplib::Headers headers;
    auto res = pimpl_->cli_.Post(
        "/containers/" + container_id + "/attach?stdout=1&stderr=1&stream=1",
        headers,
        "",
        "application/vnd.docker.raw-stream",
        [this, &container_id](const char *data, size_t data_length) {
            std::string output(data, data_length);
            if (pimpl_->onStdoutCallbacks_.contains(container_id)) pimpl_->onStdoutCallbacks_[container_id](output);
            if (pimpl_->onStderrCallbacks_.contains(container_id)) pimpl_->onStderrCallbacks_[container_id](output);
            return true;
        },
        nullptr
    );
    if (!res || res->status != 200) {
        throw std::runtime_error("Failed to attach to container");
    }
}