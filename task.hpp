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
    Session* session = nullptr;

    Task(const std::string& id, const std::string& tarBinaryData);
    ~Task();

    void tryConnection(const std::string& init, Socket::Connection* connection);

    void stop();

    std::string getToken();
};