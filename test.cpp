#include <httplib.h>
#include <iostream>
#include <nlohmann/json.hpp>

struct Impl {
    httplib::Client cli_;
    std::map<std::string, std::function<void(const std::string&)>> onStdoutCallbacks_;
    std::map<std::string, std::function<void(const std::string&)>> onStderrCallbacks_;

    explicit Impl(const std::string& socket_path) : cli_(socket_path) {}

    void createContainer() {
        nlohmann::json body = {
            {"Image", "ubuntu:latest"},
            {"OpenStdin", true},
            {"Tty", true},
            {"Cmd", {"echo", "test"}}
            // "AttachStdout" and "AttachStderr" may not be needed here
        };

        auto res = cli_.Post("/containers/create", body.dump(), "application/json");
        if (!res || res->status != 201) {
            throw std::runtime_error("Failed to create container: " + (res ? res->body : "No response"));
        }
        auto json_res = nlohmann::json::parse(res->body);
        std::string container_id = json_res["Id"];
        std::cout << "Container created with ID: " << container_id << std::endl;

        startContainer(container_id);
        attachContainer(container_id);
        // std::this_thread::sleep_for(std::chrono::milliseconds(1000));
        // getContainerLogs(container_id);
    }

    void startContainer(const std::string& container_id) {
        std::string path = "/containers/" + container_id + "/start";
        auto res = cli_.Post(path.c_str(), "", "application/json");
        if (!res || res->status != 204) {
            throw std::runtime_error("Failed to start container: " + (res ? res->body : "No response"));
        }
        std::cout << "Container " << container_id << " started" << std::endl;
    }

    void attachContainer(const std::string& container_id) {
        std::string path = "/containers/" + container_id + "/attach?stdout=1&stderr=1&stream=1";
        // Note: httplib may not support streaming out of the box; this is a simplified example
        auto res = cli_.Post(path.c_str(), "", "application/json");
        if (!res) {
            throw std::runtime_error("Failed to attach to container");
        }
        std::cout << "Attached to container " << container_id << ". Output:\n" << res->body << std::endl;
        // Process res->body as a stream for stdout and stderr
    }

    void getContainerLogs(const std::string& container_id) {
        std::string path = "/containers/" + container_id + "/logs?stdout=1&stderr=1";
        auto res = cli_.Get(path.c_str()); // Replace cli_ with your HTTP client object
        if (!res || res->status != 200 | res->body.empty()) {
            throw std::runtime_error("Failed to retrieve logs: " + (res ? res->body : "No response"));
        }
        std::string logs = res.value().body;
        std::cout << "Container logs:\n" << logs << std::endl;
    }
};

int main() {
    try {
        Impl pimpl_("http://localhost:8888");
        pimpl_.createContainer();
    } catch (const std::exception& e) {
        std::cerr << "Error: " << e.what() << std::endl;
        return 1;
    }
    return 0;
}