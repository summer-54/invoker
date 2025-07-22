#ifndef PODMAN_CLIENT_HPP
#define PODMAN_CLIENT_HPP

#include <string>
#include <vector>
#include <map>
#include <functional>

class PodmanClient {
public:
    PodmanClient(const std::string& socket_path);
    ~PodmanClient();

    // Build an image with context (path to tarball) and dockerfilePath (path within tarball)
    void build_image(const std::string& context, const std::string& dockerfilePath, const std::string& tag);

    // Create a container with specified ports, environment variables, volumes, and initial stdin
    std::string create_container(const std::string& image, const std::vector<std::string>& cmd,
                                 const std::map<std::string, std::string>& ports,
                                 const std::map<std::string, std::string>& env,
                                 const std::vector<std::pair<std::string, std::string>>& volumes,
                                 const std::string& initStdin);

    void start_container(const std::string& container_id);
    std::string run_container(const std::string& image, const std::vector<std::string>& cmd);
    void stop_container(const std::string& container_id);
    void restart_container(const std::string& container_id);
    void write_to_container_stdin(const std::string& container_id, const std::string& input);
    void setOnStdoutCallback(std::function<void(const std::string&)> callback);
    void setOnStderrCallback(std::function<void(const std::string&)> callback);
    void attach_to_container(const std::string& container_id);

private:
    struct Impl;
    std::unique_ptr<Impl> pimpl_;
};

#endif // PODMAN_CLIENT_HPP