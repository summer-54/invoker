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

    void stop();

    std::string getToken();
};