#pragma once
#include <string>

#include "session.hpp"

extern const std::string SOCKET_PATH;

class Task {
protected:
    const std::string& id;
    Session* session = nullptr;
    std::string initToken, operatorContainer;

public:
    Task(const std::string& id, const std::string& tarBinaryData);
    ~Task();

    void tryConnection(const std::string& init, Socket::Connection* connection);

    void stop();
};