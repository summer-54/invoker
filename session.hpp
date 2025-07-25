#pragma once

#include <vector>
#include <string>
#include <map>
#include "lib/podmanClient.hpp"
#include "lib/lib/socket.hpp"

extern PodmanClient podmanClient;

class Session {
protected:
    static int sessionsCount;

    int id;
    std::map<int, std::string> images, containers;
    std::map<std::string, int> revImages, revContainers;

public:
    explicit Session(int id = sessionsCount++);

    void onData(std::string data);

    void build(int image, const std::string& context, const std::string& dockerfilePath);

    void run(int id, int image, const std::string& stdout, const std::string& stderr, const std::vector<int>& ports,
        const std::vector<std::pair<std::string, std::string>>& volumes, const std::map<std::string, std::string>& env,
        const std::string& initStdin);

    void restart(int id);

    void stop(int id);

    void write(int id, const std::string& chunk);

    void port(int id, int port);

    void verdict(int id, const std::string& sub, const std::string& data);
};