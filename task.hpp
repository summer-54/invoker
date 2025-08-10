#pragma once
#include <string>

#include "session.hpp"

extern const std::string SOCKET_PATH;
extern const std::string SOCKET_INNER_PATH;
extern const std::string VOLUMES_ROOT;

class Task {
protected:
    const std::string& id;
    std::string initToken, operatorContainer, volumePath;
    std::map<std::string, std::string> networks;

public:
    std::shared_ptr<Session*> session = nullptr;

    Task(const std::string& id, const std::string& tarBinaryData);
    ~Task();

    void stop();

    std::string getToken();

    std::map<std::string, std::string> getNetworks();

    std::string getVolumePath();
};