#include <httplib.h>
#include <iostream>
#include <nlohmann/json.hpp>
#include <future>
#include <thread>
#include <string>
#include <curl/curl.h>
#include <cstdint>
#include <cstring>

struct Impl {
    httplib::Client cli_;
    std::map<std::string, std::function<void(const std::string&)>> onStdoutCallbacks_;
    std::map<std::string, std::function<void(const std::string&)>> onStderrCallbacks_;

    explicit Impl(const std::string& socket_path) : cli_(socket_path) {
        curl_global_init(CURL_GLOBAL_ALL);
    }

    ~Impl() {
        curl_global_cleanup();
    }

    void createContainer() {
        nlohmann::json body = {
            {"Image", "docker.io/oven/bun:canary"},
            {"OpenStdin", true},
            {"Tty", true},
            {"Cmd", {"bun", "-e", "process.stdout.write('test\\n'); process.stdout.write('test0\\n');"}}
        };

        auto res = cli_.Post("/containers/create", body.dump(), "application/json");
        if (!res || res->status != 201) {
            throw std::runtime_error("Failed to create container: " + (res ? res->body : "No response"));
        }
        auto json_res = nlohmann::json::parse(res->body);
        std::string container_id = json_res["Id"];
        std::cout << "Container created with ID: " << container_id << std::endl;

        // Register a stdout callback for debugging
        onStdoutCallbacks_["debug"] = [](const std::string& output) {
            std::cout << "Stdout received: " << output << std::endl;
        };

        auto attach_future = attachContainer(container_id);
        std::this_thread::sleep_for(std::chrono::milliseconds(100));
        startContainer(container_id);
        attach_future.wait();
    }

    void startContainer(const std::string& container_id) {
        std::string path = "/containers/" + container_id + "/start";
        auto res = cli_.Post(path.c_str(), "", "application/json");
        if (!res || res->status != 204) {
            throw std::runtime_error("Failed to start container: " + (res ? res->body : "No response"));
        }
        std::cout << "Container " << container_id << " started" << std::endl;
    }

    std::future<void> attachContainer(const std::string& container_id) {
        return std::async(std::launch::async, [this, container_id]() {
            struct AttachmentState {
                Impl* impl;
                std::string buffer;
            };
            AttachmentState state{this, ""};
            CURL* curl = curl_easy_init();
            if (!curl) {
                throw std::runtime_error("Failed to initialize curl");
            }

            std::string url = "http://localhost:8888/containers/" + container_id + "/attach?stdout=1&stderr=1&stream=1";
            curl_easy_setopt(curl, CURLOPT_URL, url.c_str());
            curl_easy_setopt(curl, CURLOPT_POST, 1L);
            curl_easy_setopt(curl, CURLOPT_WRITEFUNCTION, staticWriteCallback);
            curl_easy_setopt(curl, CURLOPT_WRITEDATA, &state);
            // Enable TCP keep-alive
            curl_easy_setopt(curl, CURLOPT_TCP_KEEPALIVE, 1L);
            curl_easy_setopt(curl, CURLOPT_TCP_KEEPIDLE, 120L);
            curl_easy_setopt(curl, CURLOPT_TCP_KEEPINTVL, 60L);
            // Set headers for attach protocol
            struct curl_slist* headers = nullptr;
            headers = curl_slist_append(headers, "Connection: Upgrade");
            headers = curl_slist_append(headers, "Upgrade: tcp");
            curl_easy_setopt(curl, CURLOPT_HTTPHEADER, headers);
            // Enable verbose output for debugging
            curl_easy_setopt(curl, CURLOPT_VERBOSE, 1L);

            std::cout << "Starting curl perform for attach to container " << container_id << std::endl;
            CURLcode res = curl_easy_perform(curl);
            if (res != CURLE_OK) {
                std::cerr << "curl_easy_perform failed: " << curl_easy_strerror(res) << std::endl;
                curl_slist_free_all(headers);
                curl_easy_cleanup(curl);
                throw std::runtime_error("curl_easy_perform() failed: " + std::string(curl_easy_strerror(res)));
            }
            std::cout << "curl perform completed for container " << container_id << std::endl;

            curl_slist_free_all(headers);
            curl_easy_cleanup(curl);
        });
    }

    void getContainerLogs(const std::string& container_id) {
        std::string path = "/containers/" + container_id + "/logs?stdout=1&stderr=1";
        auto res = cli_.Get(path.c_str());
        if (!res || res->status != 200 | res->body.empty()) {
            throw std::runtime_error("Failed to retrieve logs: " + (res ? res->body : "No response"));
        }
        std::string logs = res.value().body;
        std::cout << "Container logs:\n" << logs << std::endl;
    }

private:
    struct AttachmentState {
        Impl* impl;
        std::string buffer;
    };

    static size_t staticWriteCallback(char* ptr, size_t size, size_t nmemb, void* userdata) {
        auto* state = static_cast<AttachmentState*>(userdata);
        return state->impl->writeCallback(state, ptr, size, nmemb);
    }

    size_t writeCallback(AttachmentState* state, char* ptr, size_t size, size_t nmemb) {
        size_t total_size = size * nmemb;
        std::cout << "Received " << total_size << " bytes in write callback" << std::endl;
        state->buffer.append(ptr, total_size);
        while (state->buffer.size() >= 8) {
            uint8_t stream_type = static_cast<uint8_t>(state->buffer[0]);
            uint32_t net_length;
            std::memcpy(&net_length, state->buffer.data() + 4, 4);
            uint32_t payload_length = networkToHost32(net_length);
            if (state->buffer.size() < 8 + payload_length) {
                break;
            }
            std::string payload = state->buffer.substr(8, payload_length);
            state->buffer.erase(0, 8 + payload_length);
            if (stream_type == 1) { // stdout
                for (const auto& cb : onStdoutCallbacks_) {
                    cb.second(payload);
                }
            } else if (stream_type == 2) { // stderr
                for (const auto& cb : onStderrCallbacks_) {
                    cb.second(payload);
                }
            }
        }
        return total_size;
    }

    uint32_t networkToHost32(uint32_t net) const {
        return ((net >> 24) & 0xFF) | ((net >> 8) & 0xFF00) | ((net << 8) & 0xFF0000) | ((net << 24) & 0xFF000000);
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