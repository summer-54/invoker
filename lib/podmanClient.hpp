#ifndef PODMAN_CLIENT_HPP
#define PODMAN_CLIENT_HPP

#include <string>
#include <vector>
#include <map>
#include <functional>
#include <memory>

class PodmanClient {
public:
    explicit PodmanClient(const std::string& socket_path);
    ~PodmanClient();

    // Build an image with context (path to folder) and dockerfilePath (path within folder)
    void build(const std::string& tag, const std::string& context, const std::string& dockerfilePath) const;

    // Create a container with specified ports, environment variables, volumes, and initial stdin
    std::string create(const std::string& image, const std::vector<std::string>& cmd,
                                 const std::map<std::string, std::string>& ports,
                                 const std::map<std::string, std::string>& env,
                                 const std::vector<std::pair<std::string, std::string>>& volumes) const;

    std::string run(const std::string& image, const std::vector<std::string>& cmd,
                                 const std::map<std::string, std::string>& ports,
                                 const std::map<std::string, std::string>& env,
                                 const std::vector<std::pair<std::string, std::string>>& volumes,
                                 const std::string& initStdin) const;

    void start(const std::string& container_id, const std::string& initStdin) const;
    void stop(const std::string& container_id) const;
    void restart(const std::string& container_id) const;
    void write(const std::string& container_id, const std::string& input) const;
    void onStdout(const std::string& container_id, std::function<void(const std::string&)> callback) const;
    void onStderr(const std::string& container_id, std::function<void(const std::string&)> callback) const;
    void attach(const std::string& container_id) const;

private:
    struct Impl;
    std::unique_ptr<Impl> pimpl_;
};

#endif // PODMAN_CLIENT_HPP