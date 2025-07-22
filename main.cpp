#include <chrono>
#include <iostream>
#include <sstream>

#include "lib/lib/socket.hpp"
#include "session.hpp"

int main() {
    std::vector<Session*> sessions;
    Socket::Server server("/tmp/mySocket");
    server.onConnect([&sessions](Socket::Connection* conn) {
        sessions.push_back(new Session(conn));
    });
    server.start();
}