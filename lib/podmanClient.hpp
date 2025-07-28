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

    void buildTar(const std::string& tag, const std::string& binaryTarData, const std::string& dockerfilePath) const;
    void build(const std::string& tag, const std::string& context, const std::string& dockerfilePath) const;

    std::string create(const std::string& image, const std::vector<std::string>& cmd,
                       const std::map<std::string, std::string>& ports,
                       const std::map<std::string, std::string>& env,
                       const std::vector<std::pair<std::string, std::string>>& volumes,
                       const std::vector<std::string>& networks) const;

    std::string run(const std::string& image, const std::vector<std::string>& cmd,
                    const std::map<std::string, std::string>& ports,
                    const std::map<std::string, std::string>& env,
                    const std::vector<std::pair<std::string, std::string>>& volumes,
                    const std::vector<std::string>& networks,
                    const std::string& initStdin) const;

    std::string getName(const std::string& id) const;

    void start(const std::string& container_id, const std::string& initStdin) const;
    void stop(const std::string& container_id) const;
    void restart(const std::string& container_id) const;
    void write(const std::string& container_id, const std::string& input) const;
    void onStdout(const std::string& container_id, std::function<void(const std::string&)> callback) const;
    void onStderr(const std::string& container_id, std::function<void(const std::string&)> callback) const;
    void attach(const std::string& container_id) const;
    void createNetwork(const std::string& name) const;

private:
    struct Impl;
    std::unique_ptr<Impl> pimpl_;
};

#endif // PODMAN_CLIENT_HPP