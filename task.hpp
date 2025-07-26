#pragma once
#include <string>

#include "session.hpp"

extern const std::string SOCKET_PATH;
extern const std::string SOCKET_INNER_PATH;

class Task {
protected:
    const std::string& id;
    std::string initToken, operatorContainer;

public:
    std::shared_ptr<Session*> session = nullptr;
    std::map<std::string, std::string> networks;

    Task(const std::string& id, const std::string& tarBinaryData);
    ~Task();

    void stop();

    std::string getToken();
};